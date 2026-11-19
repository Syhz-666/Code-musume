use crate::{
    banter_api::{self, Failure, SummaryResponse},
    banter_credentials::Credentials,
    banter_diagnostics::{self, BanterDiagnostics, Trace},
    claude_progress::Transcript,
};
use code_musume_contracts::{
    Agent, BanterConfig, BanterKind, BanterProjection, BanterStatus, BanterUsage, DisplayState,
    EnvelopeV2, VisibleProgress,
};
use code_musume_core::{
    adapter::{normalize, Kind},
    banter::{Banter, CompletionGate, Dispatch, Entry, IdleGate, QuietActivity, Target},
    Engine,
};
use code_musume_hook::runtime::{atomic_write, config_dir};
use std::{collections::BTreeMap, fs};
use tokio::sync::{mpsc, watch};

pub type Outcome = Result<SummaryResponse, Failure>;

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
    diagnostics: BanterDiagnostics,
    request_trace: Option<(Trace, u64)>,
    work_gate: &'static str,
    #[cfg(feature = "e2e")]
    dice: std::collections::VecDeque<u8>,
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
        let mut core = Banter::new(instance, enabled);
        core.reset_idle(config.idle_enabled && has_key);
        capture.send_replace(enabled.then_some(1));
        Self {
            config,
            core,
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
            diagnostics: BanterDiagnostics::default(),
            request_trace: None,
            work_gate: "no-current-turn",
            #[cfg(feature = "e2e")]
            dice: Default::default(),
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

    pub fn outcome(&self) -> &str {
        &self.outcome
    }
    pub fn save(
        &mut self,
        mut config: BanterConfig,
        key: Option<String>,
        delete: bool,
        now: u64,
    ) -> Result<BanterStatus, String> {
        banter_api::validate(&mut config)?;
        if let Some(key) = &key {
            if key.is_empty() || key.len() > 2048 || !key.bytes().all(|b| (33..=126).contains(&b)) {
                return Err("API Key 格式无效".into());
            }
        }
        if delete {
            config.enabled = false;
            config.idle_enabled = false;
        }
        let previous =
            if config.base_url.is_empty() || (!config.active() && key.is_none() && !delete) {
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
                    && !config.active());
        if config.active() && !has_key {
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
        let work_changed = self.config.enabled != config.enabled;
        let idle_changed = self.config.idle_enabled != config.idle_enabled;
        let shared_changed = changed_key
            || BanterConfig {
                enabled: config.enabled,
                idle_enabled: config.idle_enabled,
                ..self.config.clone()
            } != config;
        self.config = config;
        self.has_key = has_key;
        if (shared_changed || idle_changed)
            && self.core.reset_idle(self.config.idle_enabled && has_key)
        {
            self.abort_task(now);
        }
        if shared_changed || work_changed {
            self.clear_work(now);
        }
        if shared_changed || work_changed {
            self.auth_failed = false;
            self.cooldown = 0;
            self.usage = None;
            self.outcome = "尚未请求".into();
        }
        Ok(self.status())
    }
    pub fn clear(&mut self, now: u64) {
        if self
            .core
            .reset_idle(self.config.idle_enabled && self.has_key)
        {
            self.abort_task(now);
        }
        self.clear_work(now);
    }
    fn clear_work(&mut self, now: u64) {
        if !self.core.has_idle_request() {
            self.abort_task(now);
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
        if !config.active() {
            config.enabled = true;
        }
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
        config.idle_enabled = self.config.idle_enabled;
        if generation == self.generation && config == self.config {
            self.auth_failed = false;
            self.cooldown = 0;
            self.usage = Some(usage);
            self.outcome = "连接测试成功".into();
        }
    }
    pub fn hook(&mut self, envelope: &EnvelopeV2, engine: &Engine, applied: bool) {
        if !self.config.enabled || !self.has_key || !applied {
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
            Kind::Prompt | Kind::SessionEnd | Kind::Error | Kind::Interrupt
        ) {
            self.core.discard_final(event.agent, &event.session);
        }
        if event.agent != Agent::ClaudeCode {
            return;
        }
        if event.kind == Kind::Done {
            // Stop can precede the final JSONL record. refresh retains only the accepted turn.
            return;
        }
        if matches!(event.kind, Kind::SessionEnd | Kind::Error | Kind::Interrupt) {
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
    pub fn completed(&mut self, target: Target, now: u64) {
        self.diagnostics.record(
            Trace::new(
                "completion",
                "turn",
                if self.config.enabled && self.has_key {
                    "awaiting-final"
                } else {
                    "disabled-or-missing-key"
                },
            )
            .conversation(target.0, &target.1, &target.2),
            now,
        );
        if self.core.completed(target, now) {
            self.abort_task(now);
        }
    }

    fn abort_task(&mut self, now: u64) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        if let Some((trace, started)) = self.request_trace.take() {
            self.diagnostics.record(
                trace
                    .outcome("request", "cancelled")
                    .elapsed(now.saturating_sub(started)),
                now,
            );
        }
    }

    fn model_gate(&self, now: u64) -> &'static str {
        if !self.config.active() {
            "disabled"
        } else if !self.has_key {
            "missing-key"
        } else if self.client.is_none() {
            "client-unavailable"
        } else if self.auth_failed {
            "auth-paused"
        } else if now < self.cooldown {
            "cooldown"
        } else {
            "ready"
        }
    }

    pub fn diagnostics(&self, now: u64) -> serde_json::Value {
        serde_json::json!({
            "trace": self.diagnostics.snapshot(),
            "workGate": self.work_gate,
            "modelGate": self.model_gate(now),
            "inFlight": self.request_trace.as_ref().map(|(trace, started)| trace.clone().elapsed(now.saturating_sub(*started))),
            "completionWaitingForRequest": self.core.completion_waiting_for_request(),
        })
    }

    pub fn rendered(&mut self, instance_id: &str, revision: u64, now: u64) {
        let view = self.core.snapshot();
        if view.instance_id == instance_id
            && view.revision == revision
            && view.expires_at_ms.is_some_and(|deadline| now < deadline)
        {
            self.diagnostics.rendered(revision, now);
        }
    }

    pub fn completion_diagnostics(&self) -> serde_json::Value {
        serde_json::json!({
            "phase": self.core.completion_phase,
            "requests": self.core.completion_requests,
            "shown": self.core.completion_shown,
        })
    }

    pub fn progress(&mut self, generation: u64, engine: &Engine, p: VisibleProgress, now: u64) {
        let trace = Trace::new(
            banter_diagnostics::kind(p.kind),
            "source-message",
            "received",
        )
        .conversation(p.agent, &p.session, &p.turn);
        let disposition = if generation != self.generation {
            "stale-capture-generation"
        } else if !self.config.enabled {
            "work-banter-disabled"
        } else if engine.active_turn(p.agent, &p.session) == Some(p.turn.as_str())
            || p.kind == BanterKind::Completion
                && self.core.accepts_final(p.agent, &p.session, &p.turn, now)
        {
            if p.agent == Agent::Codex {
                self.codex_ready = true;
            }
            self.core.ingest(p, now)
        } else {
            "outside-active-turn-or-final-window"
        };
        self.diagnostics
            .record(trace.outcome("source-message", disposition), now);
    }
    pub fn reconcile_idle(&mut self, engine: &Engine, paused: bool, now: u64) {
        let gate = if !engine.idle_banter_available() {
            IdleGate::Blocked
        } else if paused {
            IdleGate::Paused
        } else {
            IdleGate::Ready
        };
        if self.core.update_idle(gate, now) {
            self.abort_task(now);
            self.outcome = "空闲碎碎念已暂停".into();
        }
    }

    #[cfg(feature = "e2e")]
    pub fn set_idle_dice(&mut self, faces: Vec<u8>) -> Result<(), String> {
        if faces.iter().any(|face| !(1..=6).contains(face)) {
            return Err("骰子点数须为 1 到 6".into());
        }
        self.dice = faces.into();
        Ok(())
    }

    pub fn refresh(
        &mut self,
        engine: &Engine,
        preview: bool,
        now: u64,
        dispatch: bool,
        idle_provider_available: Option<bool>,
    ) -> BanterProjection {
        self.core
            .retain(|key| engine.tracked_sessions().any(|tracked| tracked == key));
        let finishing = self.core.completion_target().cloned();
        self.core.retain_finals(|agent, session, turn| {
            engine.active_turn(agent, session) == Some(turn)
                || finishing
                    .as_ref()
                    .is_some_and(|t| t.0 == agent && t.1 == session && t.2 == turn)
        });
        self.transcripts.retain(|id, t| {
            engine.active_turn(Agent::ClaudeCode, id) == Some(t.turn())
                || self
                    .core
                    .accepts_final(Agent::ClaudeCode, id, t.turn(), now)
        });
        if dispatch && now.saturating_sub(self.last_poll) >= 500 {
            self.last_poll = now;
            let mut batch = Vec::new();
            for (id, t) in &mut self.transcripts {
                if let Ok(messages) = t.read(id) {
                    batch.extend(messages);
                }
            }
            for message in batch {
                self.progress(self.generation, engine, message, now);
            }
        }
        let snapshot = engine.snapshot();
        let work = (!preview)
            .then(|| engine.banter_work(self.core.work_target(), now))
            .flatten();
        let covered_by_done = snapshot.state == DisplayState::Done;
        self.work_gate = if !self.config.enabled {
            "disabled"
        } else if preview {
            "manual-preview"
        } else if covered_by_done || work.is_none() {
            "no-eligible-work-target"
        } else if matches!(work, Some((_, QuietActivity::AwaitingUpdate))) {
            "awaiting-fresh-work-signal"
        } else {
            "eligible"
        };
        let ready = self.model_gate(now) == "ready";
        let dispatch = if !dispatch {
            Dispatch::Observe
        } else if ready {
            Dispatch::Ready
        } else {
            Dispatch::Unavailable
        };
        let gate = self
            .core
            .completion_target()
            .map_or(CompletionGate::Blocked, |target| {
                if preview {
                    CompletionGate::Blocked
                } else {
                    engine.completion_gate(target, now)
                }
            });
        let before = self.core.snapshot().revision;
        let phase = self.core.completion_phase;
        let (completion, abort) = self
            .core
            .tick_completion(gate, now, dispatch == Dispatch::Ready);
        if abort {
            self.abort_task(now);
        }
        if self.core.completion_phase != phase && completion.is_none() {
            let mut trace = Trace::new("completion", "turn", self.core.completion_phase);
            if let Some(Target(agent, session, turn)) = &finishing {
                trace = trace.conversation(*agent, session, turn);
            }
            self.diagnostics.record(trace, now);
        }
        if phase != "shown" && self.core.completion_phase == "shown" {
            self.outcome = "完成小结已显示".into();
        }
        let work_dispatch = if covered_by_done {
            Dispatch::Observe
        } else {
            dispatch
        };
        let progress = self.core.tick(work, now, work_dispatch);
        let request = completion.or(progress).or_else(|| {
            let idle_dispatch = if !idle_provider_available? && dispatch == Dispatch::Ready {
                Dispatch::Unavailable
            } else {
                dispatch
            };
            self.core.tick_idle(now, idle_dispatch, || {
                #[cfg(feature = "e2e")]
                if let Some(face) = self.dice.pop_front() {
                    return face;
                }
                fastrand::u8(1..=6)
            })
        });
        let view = self.core.snapshot();
        if view.revision != before {
            let context = if let Some(Target(agent, session, turn)) = &finishing {
                (view.kind == Some(BanterKind::Completion)).then(|| {
                    Trace::new("completion", "bubble", "published")
                        .conversation(*agent, session, turn)
                })
            } else {
                None
            };
            self.diagnostics.publication(&view, context, now);
        }
        if let Some(request) = request {
            let (id, kind, target) = self
                .core
                .request_context()
                .expect("scheduled model request");
            let trace = Trace::request(id, kind, target);
            match self.key_for(&self.config) {
                Ok(key) => {
                    self.diagnostics.record(trace.clone(), now);
                    self.request_trace = Some((trace, now));
                    self.outcome = if self.core.has_idle_request() {
                        "正在生成空闲碎碎念"
                    } else if self.core.quiet_activity(request.id).is_some() {
                        "正在生成陪伴短句"
                    } else {
                        "正在请求总结"
                    }
                    .into();
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
                    self.diagnostics.record(
                        trace.clone().outcome("request", "credential-unavailable"),
                        now,
                    );
                    self.core.finish(request.id, None, now);
                    let after = self.core.snapshot();
                    if after.revision != view.revision {
                        self.diagnostics.publication(&after, Some(trace), now);
                    }
                    self.outcome = error;
                    self.auth_failed = true;
                }
            }
        }
        self.core.snapshot()
    }
    pub fn finish(
        &mut self,
        id: u64,
        result: Outcome,
        now: u64,
        engine: &Engine,
        window_visible: bool,
    ) {
        let before = self.core.snapshot().revision;
        let quiet = self.core.quiet_activity(id);
        let idle = self.core.has_idle_request();
        let usable = match self.core.request_context() {
            Some((current, "completion", Some(target))) if current == id => {
                engine.completion_gate(target, now) != CompletionGate::Blocked
            }
            Some((current, "idle", _)) if current == id => window_visible,
            Some((current, _, Some(target))) if current == id => engine
                .banter_work(self.core.work_target(), now)
                .is_some_and(|(work, activity)| {
                    &work == target
                        && (activity != QuietActivity::AwaitingUpdate || quiet == Some(activity))
                }),
            _ => false,
        };
        let text = usable
            .then(|| {
                result
                    .as_ref()
                    .ok()
                    .and_then(|response| response.line.as_ref().ok().cloned())
            })
            .flatten();
        let Some(disposition) = self.core.finish(id, text, now) else {
            self.diagnostics.record(
                Trace::request(id, "unmatched", None).outcome("result", "late-or-cancelled"),
                now,
            );
            return;
        };
        if let Some((trace, started)) = self.request_trace.take() {
            let view = self.core.snapshot();
            let disposition = if usable {
                disposition
            } else {
                "display-not-eligible"
            };
            self.diagnostics.record(
                trace
                    .clone()
                    .outcome("result", disposition)
                    .elapsed(now.saturating_sub(started))
                    .provider_error(result.as_ref().err().map(Failure::label))
                    .output_rejection(result.as_ref().ok().and_then(|response| {
                        response
                            .line
                            .as_ref()
                            .err()
                            .map(|rejection| rejection.code())
                    })),
                now,
            );
            if view.revision != before {
                self.diagnostics.publication(&view, Some(trace), now);
            }
        }
        self.task = None;
        match result {
            Ok(SummaryResponse { line, usage }) => {
                self.outcome = if idle {
                    if self.core.snapshot().revision != before {
                        "空闲碎碎念已显示"
                    } else {
                        "本次空闲碎碎念已跳过"
                    }
                } else if quiet.is_some() && self.core.snapshot().revision != before {
                    if line.is_ok() {
                        "陪伴短句已显示"
                    } else {
                        "无合格的陪伴短句，已显示本地提示"
                    }
                } else if self.core.snapshot().revision != before {
                    "总结成功"
                } else if self.core.completion_phase == "ready" {
                    "完成小结已生成"
                } else if line.is_ok() {
                    "已收到总结，当前无法展示"
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
    let entries = if config.idle_enabled && !config.enabled {
        [Entry::Idle { recent: vec![] }]
    } else {
        [Entry::Progress(
            "测试样例：登录测试已通过，保存设置的测试还在运行。".into(),
        )]
    };
    let SummaryResponse { line, usage } = banter_api::summarize(&client, &config, &key, &entries)
        .await
        .map_err(|e| e.label().to_owned())?;
    Ok((line.map_err(|_| "服务没有返回合格短句")?, usage))
}
