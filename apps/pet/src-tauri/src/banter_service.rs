use crate::{
    banter_api::{self, Failure},
    banter_credentials::Credentials,
    claude_progress::Transcript,
};
use code_musume_contracts::{
    Agent, BanterConfig, BanterProjection, BanterStatus, BanterUsage, EnvelopeV2, VisibleProgress,
};
use code_musume_core::{
    adapter::{normalize, Kind},
    banter::{Banter, Entry, Target},
    Engine,
};
use code_musume_hook::runtime::{atomic_write, config_dir};
use std::{collections::BTreeMap, fs};
use tokio::sync::{mpsc, watch};

pub type Outcome = Result<(Option<String>, BanterUsage), Failure>;

pub struct Service {
    pub config: BanterConfig,
    pub core: Banter,
    pub generation: u64,
    capture: watch::Sender<Option<u64>>,
    credentials: Credentials,
    transcripts: BTreeMap<String, Transcript>,
    client: Option<reqwest::Client>,
    outcomes: mpsc::Sender<(u64, Outcome)>,
    task: Option<tauri::async_runtime::JoinHandle<()>>,
    last_poll: u64,
    cooldown: u64,
    auth_failed: bool,
    outcome: String,
    usage: Option<BanterUsage>,
    has_key: bool,
    codex_ready: bool,
    claude_ready: bool,
}

impl Service {
    pub fn new(
        instance: String,
        mode: &str,
        capture: watch::Sender<Option<u64>>,
        outcomes: mpsc::Sender<(u64, Outcome)>,
    ) -> Self {
        let mut config: BanterConfig = fs::read(config_dir().join("banter.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        if banter_api::validate(&mut config).is_err() {
            config = BanterConfig::default();
        }
        let credentials = Credentials::new(mode);
        let has_key =
            !config.base_url.is_empty() && credentials.read(&config).ok().flatten().is_some();
        let enabled = config.enabled && has_key;
        capture.send_replace(enabled.then_some(1));
        Self {
            config,
            core: Banter::new(instance, enabled),
            generation: 1,
            capture,
            credentials,
            transcripts: BTreeMap::new(),
            client: banter_api::client().ok(),
            outcomes,
            task: None,
            last_poll: 0,
            cooldown: 0,
            auth_failed: false,
            outcome: "尚未请求".into(),
            usage: None,
            has_key,
            codex_ready: false,
            claude_ready: false,
        }
    }
    pub fn status(&self) -> BanterStatus {
        BanterStatus {
            config: self.config.clone(),
            has_key: self.has_key,
            codex_ready: self.codex_ready,
            claude_ready: self.claude_ready,
            outcome: self.outcome.clone(),
            usage: self.usage.clone(),
            skipped_long: self.core.skipped_long,
        }
    }
    pub fn save(
        &mut self,
        mut config: BanterConfig,
        key: Option<String>,
        delete: bool,
    ) -> Result<BanterStatus, String> {
        banter_api::validate(&mut config)?;
        if let Some(key) = &key {
            if key.is_empty() || key.len() > 2048 || !key.bytes().all(|b| (33..=126).contains(&b)) {
                return Err("API Key 格式无效".into());
            }
        }
        if delete {
            config.enabled = false;
        }
        let previous =
            if config.base_url.is_empty() || (!config.enabled && key.is_none() && !delete) {
                None
            } else {
                self.credentials.read(&config)?
            };
        let has_key = !delete
            && (key.is_some()
                || previous.is_some()
                || config.protocol == self.config.protocol
                    && config.base_url == self.config.base_url
                    && self.has_key
                    && !config.enabled);
        if config.enabled && !has_key {
            return Err("请填写 API Key".into());
        }
        let changed_key = key.is_some() || delete;
        if changed_key {
            banter_api::endpoint(&config)?;
            self.credentials
                .store(&config, key.as_deref().filter(|_| !delete))?;
        }
        if let Err(error) = atomic_write(
            &config_dir().join("banter.json"),
            &serde_json::to_vec_pretty(&config).map_err(|_| "无法保存碎碎念设置")?,
        ) {
            if changed_key {
                let _ = self.credentials.store(&config, previous.as_deref());
            }
            return Err(error);
        }
        let changed = self.config != config || changed_key;
        self.config = config;
        self.has_key = has_key;
        if changed {
            self.clear();
            self.auth_failed = false;
            self.cooldown = 0;
            self.usage = None;
            self.outcome = "尚未请求".into();
        }
        Ok(self.status())
    }
    pub fn clear(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.generation += 1;
        let enabled = self.config.enabled && self.has_key;
        self.capture
            .send_replace(enabled.then_some(self.generation));
        self.transcripts.clear();
        self.claude_ready = false;
        self.codex_ready = false;
        self.core.reset(enabled);
    }
    pub fn key_for(&self, config: &BanterConfig) -> Result<String, String> {
        self.credentials
            .read(config)?
            .ok_or_else(|| "请填写 API Key".into())
    }
    pub fn test_call(
        &self,
        mut config: BanterConfig,
        key: Option<String>,
    ) -> Result<(reqwest::Client, BanterConfig, String, u64), String> {
        config.enabled = true;
        banter_api::validate(&mut config)?;
        let key = match key {
            Some(key) if !key.is_empty() => key,
            _ => self.key_for(&config)?,
        };
        if key.len() > 2048 || !key.bytes().all(|b| (33..=126).contains(&b)) {
            return Err("API Key 格式无效".into());
        }
        Ok((
            self.client.clone().ok_or("无法初始化总结服务")?,
            config,
            key,
            self.generation,
        ))
    }
    pub fn test_ok(&mut self, generation: u64, mut config: BanterConfig, usage: BanterUsage) {
        config.enabled = self.config.enabled;
        if generation == self.generation && config == self.config {
            self.auth_failed = false;
            self.cooldown = 0;
            self.usage = Some(usage);
            self.outcome = "连接测试成功".into();
        }
    }
    pub fn hook(&mut self, envelope: &EnvelopeV2, engine: &Engine) {
        if !self.config.enabled || !self.has_key || envelope.agent != Agent::ClaudeCode {
            return;
        }
        let Ok(Some(event)) = normalize(envelope) else {
            return;
        };
        if event.actor.is_some() {
            return;
        }
        if matches!(
            event.kind,
            Kind::SessionEnd | Kind::Done | Kind::Error | Kind::Interrupt
        ) {
            if engine.active_turn(event.agent, &event.session).is_none() {
                self.transcripts.remove(&event.session);
            }
            return;
        }
        let Some(turn) = engine.active_turn(event.agent, &event.session) else {
            return;
        };
        if self
            .transcripts
            .get(&event.session)
            .is_some_and(|t| t.turn() == turn)
        {
            return;
        }
        let Ok(payload) = serde_json::from_str::<serde_json::Value>(&envelope.payload) else {
            return;
        };
        let Some(path) = payload["transcript_path"].as_str() else {
            return;
        };
        if let Some(transcript) = Transcript::open(path, &event.session, turn) {
            self.transcripts.insert(event.session, transcript);
            self.claude_ready = true;
        }
    }
    pub fn progress(&mut self, generation: u64, engine: &Engine, p: VisibleProgress, now: u64) {
        if generation == self.generation
            && self.config.enabled
            && engine.active_turn(p.agent, &p.session) == Some(p.turn.as_str())
        {
            if p.agent == Agent::Codex {
                self.codex_ready = true;
            }
            self.core.ingest(p, now);
        }
    }
    pub fn refresh(
        &mut self,
        engine: &Engine,
        preview: bool,
        now: u64,
        dispatch: bool,
    ) -> BanterProjection {
        self.core
            .retain(|key| engine.tracked_sessions().any(|tracked| tracked == key));
        self.transcripts
            .retain(|id, t| engine.active_turn(Agent::ClaudeCode, id) == Some(t.turn()));
        if dispatch && now.saturating_sub(self.last_poll) >= 500 {
            self.last_poll = now;
            for (id, t) in &mut self.transcripts {
                if let Ok(messages) = t.read(id) {
                    for message in messages {
                        self.core.ingest(message, now);
                    }
                }
            }
        }
        let target = (!preview)
            .then(|| engine.banter_target())
            .flatten()
            .map(|(a, s, t)| Target(a, s.into(), t.into()));
        let can_request = dispatch
            && engine.snapshot().certainty == code_musume_contracts::Certainty::Observed
            && self.client.is_some()
            && self.has_key
            && !self.auth_failed
            && now >= self.cooldown;
        if let Some(request) = self.core.tick(target, now, can_request) {
            match self.key_for(&self.config) {
                Ok(key) => {
                    let client = self.client.clone().unwrap();
                    let config = self.config.clone();
                    let outcomes = self.outcomes.clone();
                    self.task = Some(tauri::async_runtime::spawn(async move {
                        let result =
                            banter_api::summarize(&client, &config, &key, &request.entries).await;
                        let _ = outcomes.send((request.id, result)).await;
                    }));
                }
                Err(error) => {
                    self.core.finish(request.id, None, now);
                    self.outcome = error;
                    self.auth_failed = true;
                }
            }
        }
        self.core.snapshot()
    }
    pub fn finish(&mut self, id: u64, result: Outcome, now: u64, observed: bool) {
        let text = observed
            .then(|| result.as_ref().ok().and_then(|(text, _)| text.clone()))
            .flatten();
        if !self.core.finish(id, text, now) {
            return;
        }
        self.task = None;
        match result {
            Ok((text, usage)) => {
                self.outcome = if text.is_some() {
                    "总结成功"
                } else {
                    "无可展示的短句"
                }
                .into();
                self.usage = Some(usage);
            }
            Err(error) => {
                self.outcome = error.label().into();
                self.auth_failed = matches!(error, Failure::Auth);
                self.cooldown = now.saturating_add(match error {
                    Failure::Limit(ms) => ms,
                    _ => 60_000,
                });
            }
        }
    }
}

pub async fn probe(
    client: reqwest::Client,
    config: BanterConfig,
    key: String,
) -> Result<(String, BanterUsage), String> {
    let entries = [Entry::Progress(
        "测试样例：登录测试已通过，保存设置的测试还在运行。".into(),
    )];
    let (text, usage) = banter_api::summarize(&client, &config, &key, &entries)
        .await
        .map_err(|e| e.label().to_owned())?;
    Ok((text.ok_or("服务没有返回合格短句")?, usage))
}
