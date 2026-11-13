use super::pipeline_diagnostics::{PipelineDiagnostics, PipelineTrace};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use code_musume_contracts::*;
use code_musume_core::{engine::CodexApprovalStatus, Engine};
use code_musume_hook::runtime::{atomic_write, config_dir, runtime_path};
use std::{
    collections::BTreeMap,
    fs,
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use tauri::{
    ipc::{Channel, JavaScriptChannelId},
    AppHandle, Emitter, Manager, Webview,
};
use tokio::sync::{mpsc, oneshot, watch};

type Inspection = (
    Projection,
    Vec<Diagnostic>,
    BTreeMap<String, CodexApprovalStatus>,
);

enum Message {
    Event(EnvelopeV2, usize, Instant),
    Subscribe(
        Box<Webview>,
        String,
        JavaScriptChannelId,
        oneshot::Sender<Projection>,
    ),
    Unsubscribe(String),
    Get(oneshot::Sender<Projection>),
    BanterSubscribe(
        Box<Webview>,
        String,
        JavaScriptChannelId,
        oneshot::Sender<BanterProjection>,
    ),
    BanterUnsubscribe(String),
    BanterGet(oneshot::Sender<BanterProjection>),
    BanterStatus(oneshot::Sender<BanterStatus>),
    BanterSave(
        BanterConfig,
        Option<String>,
        bool,
        oneshot::Sender<Result<BanterStatus, String>>,
    ),
    BanterProbe(
        BanterConfig,
        Option<String>,
        bool,
        oneshot::Sender<Result<String, String>>,
    ),
    #[cfg(feature = "e2e")]
    BanterFixture(VisibleProgress, oneshot::Sender<()>),
    Config(
        Config,
        CharacterManifest,
        oneshot::Sender<Result<Config, String>>,
    ),
    Diagnostics(oneshot::Sender<Vec<Diagnostic>>),
    Inspect(oneshot::Sender<Inspection>),
    Reset,
    Position(Position),
    PreviewEnabled(bool, oneshot::Sender<Result<AnimationPreview, String>>),
    PreviewState(
        Option<DisplayState>,
        oneshot::Sender<Result<AnimationPreview, String>>,
    ),
    #[cfg(feature = "e2e")]
    AdvanceClock(u64, oneshot::Sender<Projection>),
}
struct Clock {
    start: Instant,
    #[cfg(feature = "e2e")]
    offset: AtomicU64,
}
impl Clock {
    fn now(&self) -> u64 {
        let elapsed = self.start.elapsed().as_millis() as u64;
        #[cfg(feature = "e2e")]
        {
            elapsed.saturating_add(self.offset.load(Ordering::SeqCst))
        }
        #[cfg(not(feature = "e2e"))]
        {
            elapsed
        }
    }
}
pub struct Native {
    tx: mpsc::Sender<Message>,
    bytes: AtomicUsize,
    lost: AtomicU64,
    pub config: Mutex<Config>,
    pub runtime: RuntimeFile,
    pub runtime_path: std::path::PathBuf,
    rate: Mutex<(Instant, f64)>,
    pub render_fps: Mutex<Option<(Instant, f64)>>,
    pub render_receipts: Mutex<super::render_receipts::RenderReceipts>,
    pub animation_preview: Mutex<super::animation_preview::PreviewController>,
    pipeline: Mutex<PipelineDiagnostics>,
    codex_approval_sync: bool,
    #[cfg(windows)]
    approval_rejections: watch::Receiver<super::codex_approval::Rejections>,
    clock: Clock,
}
impl Native {
    pub fn trace(&self, entry: PipelineTrace) {
        self.pipeline
            .lock()
            .unwrap()
            .record(entry, self.clock.now());
    }
    pub async fn diagnostic_report(&self) -> Result<serde_json::Value, String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Inspect(tx))
            .await
            .map_err(|_| "核心已停止")?;
        let (projection, diagnostics, approvals) = rx.await.map_err(|_| "核心已停止")?;
        let pipeline = self.pipeline.lock().unwrap().snapshot();
        let preview = self.animation_preview.lock().unwrap().snapshot();
        let receipt = self.render_receipts.lock().unwrap().latest.clone();
        #[cfg(windows)]
        let rejected = serde_json::to_value(*self.approval_rejections.borrow()).unwrap_or_default();
        #[cfg(not(windows))]
        let rejected = serde_json::Value::Null;
        Ok(serde_json::json!({
            "v": 2, "appVersion": self.runtime.app_version,
            "instanceId": self.runtime.instance_id, "capturedAtMs": self.clock.now(),
            "platform": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "queue": {"entries": MAX_QUEUE_ENTRIES-self.tx.capacity(), "bytes": self.bytes.load(Ordering::SeqCst),
                "limitEntries": MAX_QUEUE_ENTRIES, "limitBytes": MAX_QUEUE_BYTES, "unprocessedLoss": self.lost.load(Ordering::SeqCst)},
            "projection": projection, "coreDiagnostics": diagnostics,
            "pipeline": pipeline, "preview": preview, "renderReceipt": receipt,
            "codexApprovalSync": {"enabled": self.codex_approval_sync, "sessions": approvals, "rejected": rejected},
        }))
    }
    pub fn start(app: Option<AppHandle>, mode: &str) -> Result<Arc<Self>, String> {
        #[cfg(windows)]
        let approval_pipe =
            (app.is_some() && !cfg!(feature = "e2e")).then_some(super::codex_approval::PIPE);
        #[cfg(not(windows))]
        let approval_pipe = None;
        Self::start_with_approval_pipe(app, mode, approval_pipe)
    }
    fn start_with_approval_pipe(
        app: Option<AppHandle>,
        mode: &str,
        approval_pipe: Option<&str>,
    ) -> Result<Arc<Self>, String> {
        let path = config_dir().join("config.json");
        let mut config = match fs::read(&path) {
            Ok(b) => match serde_json::from_slice::<Config>(&b)
                .ok()
                .filter(|c| c.schema_version == 1)
            {
                Some(c) => c,
                None => {
                    atomic_write(
                        &path.with_extension(format!("invalid-{}.json", uuid::Uuid::new_v4())),
                        &b,
                    )?;
                    Config::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
            Err(e) => return Err(e.to_string()),
        };
        let character = match super::validate_config(&config) {
            Ok(m) => m,
            Err(_) => {
                config.character_id = "builtin".into();
                config.form_lock = None;
                config.agent_forms.clear();
                if !matches!(config.renderer.as_str(), "pixi" | "dom") {
                    config.renderer = "pixi".into();
                }
                super::characters::builtin()
            }
        };
        let listener = (47921..47931)
            .find_map(|port| std::net::TcpListener::bind(("127.0.0.1", port)).ok())
            .ok_or("本地端口均被占用")?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let runtime = RuntimeFile {
            v: 2,
            port,
            token: format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            ),
            pid: std::process::id(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            instance_id: uuid::Uuid::new_v4().to_string(),
            started_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        };
        let (tx, mut rx) = mpsc::channel(MAX_QUEUE_ENTRIES);
        let (codex_sessions, wanted) = watch::channel(Vec::new());
        let (approval_updates, mut approvals) = mpsc::channel(MAX_QUEUE_ENTRIES);
        let (capture, capture_rx) = watch::channel(None);
        let (progress_tx, mut progress_rx) = mpsc::channel::<(u64, VisibleProgress)>(32);
        let (outcome_tx, mut outcome_rx) = mpsc::channel(4);
        let codex_approval_sync = approval_pipe.is_some();
        #[cfg(windows)]
        let (rejections, approval_rejections) = watch::channel(Default::default());
        #[cfg(windows)]
        if let Some(path) = approval_pipe.map(str::to_owned) {
            tauri::async_runtime::spawn(async move {
                super::codex_approval::observe(
                    &path,
                    wanted,
                    approval_updates,
                    rejections,
                    capture_rx,
                    Some(progress_tx),
                )
                .await;
            });
        }
        #[cfg(not(windows))]
        drop((wanted, approval_updates, capture_rx, progress_tx));
        let native = Arc::new(Self {
            clock: Clock {
                start: Instant::now(),
                #[cfg(feature = "e2e")]
                offset: AtomicU64::new(0),
            },
            render_receipts: Mutex::new(super::render_receipts::RenderReceipts::default()),
            animation_preview: Mutex::new(super::animation_preview::PreviewController::new(
                config.manual_animation_enabled,
            )),
            tx,
            pipeline: Mutex::new(PipelineDiagnostics::default()),
            codex_approval_sync,
            #[cfg(windows)]
            approval_rejections,
            bytes: AtomicUsize::new(0),
            lost: AtomicU64::new(0),
            config: Mutex::new(config.clone()),
            runtime_path: runtime_path(mode),
            runtime,
            rate: Mutex::new((Instant::now(), 200.0)),
            render_fps: Mutex::new(None),
        });
        let state = native.clone();
        let instance = native.runtime.instance_id.clone();
        let mode = mode.to_owned();
        tauri::async_runtime::spawn(async move {
            let mut banter =
                super::banter_service::Service::new(instance.clone(), &mode, capture, outcome_tx);
            let mut engine =
                Engine::new(instance, config).with_codex_approval_sync(codex_approval_sync);
            engine.character_forms = character.agent_forms;
            engine.default_form = character.default_form;
            let mut subscribers: BTreeMap<String, Channel<Projection>> = BTreeMap::new();
            let mut banter_subscribers: BTreeMap<String, Channel<BanterProjection>> =
                BTreeMap::new();
            let mut last_banter = 0;
            let mut probe_active = false;
            let (probe_done, mut probe_finished) = mpsc::channel(1);
            let mut last_broadcast = 0;
            let mut timer = tokio::time::interval(Duration::from_millis(250));
            let mut position_dirty = false;
            let mut last_position = Instant::now();
            let mut fullscreen = false;
            let mut last_fullscreen = Instant::now();
            loop {
                let now = state.clock.now();
                let mut received = None;
                let mut dispatch_banter = false;
                tokio::select! {
                    msg=rx.recv()=>match msg{
                        Some(Message::Event(e,size,at))=>{
                            received=Some(at);state.bytes.fetch_sub(size,Ordering::SeqCst);
                            let mut trace=PipelineTrace::event("core","processed",&e);
                            trace.before=Some((&engine.snapshot()).into());
                            engine.ingest(e.clone(),state.clock.now());
                            banter.hook(&e, &engine);
                            if codex_approval_sync {
                                codex_sessions.send_if_modified(|wanted| {
                                    let next = engine.codex_sessions();
                                    if *wanted == next { return false; }
                                    *wanted = next;
                                    true
                                });
                            }
                            trace.outcome=engine.latest_diagnostic().map(|d|d.outcome.clone()).unwrap_or_else(||"no-outcome".into());
                            trace.after=Some((&engine.snapshot()).into());
                            trace.elapsed_ms=Some(at.elapsed().as_secs_f64()*1000.0);
                            state.trace(trace);
                        },
                        Some(Message::Subscribe(webview, token, callback, reply)) => {
                            let label = webview.label();
                            let id = format!("{label}:{token}");
                            // IPC fallback can replay the same request. A second Channel with
                            // this callback would reset its index and close it when dropped.
                            if !subscribers.contains_key(&id) {
                                subscribers.retain(|old, _| old.split(':').next() != Some(label));
                                subscribers.insert(id, callback.channel_on(*webview));
                            }
                            let _ = reply.send(engine.snapshot());
                        },
                        Some(Message::Unsubscribe(id))=>{subscribers.remove(&id);},
                        Some(Message::BanterSubscribe(webview, token, callback, reply)) => {
                            let label = webview.label();
                            let id = format!("{label}:{token}");
                            if !banter_subscribers.contains_key(&id) {
                                banter_subscribers.retain(|old, _| old.split(':').next() != Some(label));
                                banter_subscribers.insert(id, callback.channel_on(*webview));
                            }
                            let _ = reply.send(banter.core.snapshot());
                        },
                        Some(Message::BanterUnsubscribe(id)) => { banter_subscribers.remove(&id); },
                        Some(Message::BanterGet(reply)) => { let _ = reply.send(banter.core.snapshot()); },
                        Some(Message::BanterStatus(reply)) => { let _ = reply.send(banter.status()); },
                        Some(Message::BanterSave(config, key, delete, reply)) => {
                            let _ = reply.send(banter.save(config, key, delete));
                        },
                        Some(Message::BanterProbe(config, key, preview, reply)) => {
                            if probe_active {
                                let _ = reply.send(Err("已有连接测试正在运行".into()));
                            } else {
                                match banter.test_call(config, key) {
                                    Ok((client, config, key, generation)) => {
                                        probe_active = true;
                                        let sender = probe_done.clone();
                                        tauri::async_runtime::spawn(async move {
                                            let result = super::banter_service::probe(client, config.clone(), key).await;
                                            let update = result.as_ref().ok().map(|(_, usage)| (generation, config, usage.clone()));
                                            let _ = reply.send(result.map(|(text, _)| if preview { text } else { "连接测试成功".into() }));
                                            let _ = sender.send(update).await;
                                        });
                                    },
                                    Err(error) => { let _ = reply.send(Err(error)); },
                                }
                            }
                        },
                        #[cfg(feature = "e2e")]
                        Some(Message::BanterFixture(p, reply)) => {
                            banter.progress(banter.generation, &engine, p, state.clock.now());
                            let _ = reply.send(());
                        },
                        Some(Message::Get(reply))=>{engine.tick(state.clock.now());let _=reply.send(engine.snapshot());},
                        #[cfg(feature = "e2e")]
                        Some(Message::AdvanceClock(ms,reply))=>{state.clock.offset.fetch_add(ms,Ordering::SeqCst);engine.tick(state.clock.now());let _=reply.send(engine.snapshot());},
                        Some(Message::Diagnostics(reply))=>{let _=reply.send(engine.diagnostics());},
                        Some(Message::Inspect(reply))=>{let _=reply.send((engine.snapshot(),engine.diagnostics(),engine.codex_approvals()));},
                        Some(Message::Reset)=>{
                            engine.reset(now);
                            banter.clear();
                            if codex_approval_sync { codex_sessions.send_replace(Vec::new()); }
                        },
                        Some(Message::Position(p))=>{engine.config.position=Some(p);*state.config.lock().unwrap()=engine.config.clone();position_dirty=true;last_position=Instant::now();},
                        Some(Message::PreviewEnabled(enabled, reply))=>{
                            let result=(||{
                                if engine.config.manual_animation_enabled != enabled {
                                    let mut next=engine.config.clone();
                                    next.manual_animation_enabled=enabled;
                                    atomic_write(&config_dir().join("config.json"),&serde_json::to_vec_pretty(&next).unwrap())?;
                                    engine.config=next.clone();
                                    *state.config.lock().unwrap()=next;
                                }
                                let (changed,snapshot)={let mut preview=state.animation_preview.lock().unwrap();let changed=preview.set_enabled(enabled);(changed,preview.snapshot())};
                                if changed {if let Some(app)=&app {let _=app.emit("animation-preview-changed",&snapshot);}}
                                Ok(snapshot)
                            })();
                            let _=reply.send(result);
                        },
                        Some(Message::PreviewState(selected, reply))=>{
                            let result=(||{
                                let (changed,snapshot)={let mut preview=state.animation_preview.lock().unwrap();let changed=preview.select(selected)?;(changed,preview.snapshot())};
                                if changed {if let Some(app)=&app {let _=app.emit("animation-preview-changed",&snapshot);}}
                                Ok(snapshot)
                            })();
                            let _=reply.send(result);
                        },
                        Some(Message::Config(mut c,character,reply))=>{
                            c.position=engine.config.position.clone();
                            let result=(||{
                                if c.autostart!=engine.config.autostart{super::platform::autostart(c.autostart)?;}
                                if let Err(e)=atomic_write(&config_dir().join("config.json"),&serde_json::to_vec_pretty(&c).unwrap()){
                                    if c.autostart!=engine.config.autostart{let _=super::platform::autostart(engine.config.autostart);}return Err(e);
                                }Ok(c)
                            })();
                            if let Ok(c)=&result{engine.config=c.clone();engine.character_forms=character.agent_forms;engine.default_form=character.default_form;*state.config.lock().unwrap()=c.clone();let (changed,snapshot)={let mut preview=state.animation_preview.lock().unwrap();let changed=preview.set_enabled(c.manual_animation_enabled);(changed,preview.snapshot())};if changed{if let Some(app)=&app{let _=app.emit("animation-preview-changed",&snapshot);}}engine.tick(now);if let Some(app)=&app{fullscreen=c.fullscreen_hide&&super::platform::fullscreen(app);super::apply_visibility(app,&state,fullscreen);}}
                            let _=reply.send(result);
                        },
                        None=>break,
                    },
                    Some((id, next))=approvals.recv()=>{
                        received=Some(Instant::now());
                        engine.update_codex_approvals(id,next,state.clock.now());
                    },
                    Some((generation, message)) = progress_rx.recv() => {
                        banter.progress(generation, &engine, message, state.clock.now());
                    },
                    Some((id, result)) = outcome_rx.recv() => {
                        banter.finish(id, result, state.clock.now(), engine.snapshot().certainty == Certainty::Observed);
                    },
                    Some(update) = probe_finished.recv() => {
                        probe_active = false;
                        if let Some((generation, config, usage)) = update {
                            banter.test_ok(generation, config, usage);
                        }
                    },
                    _=timer.tick()=>{
                        dispatch_banter = true;
                        let now=state.clock.now();let lost=state.lost.swap(0,Ordering::SeqCst);if lost>0{engine.lost(lost,now);}engine.tick(now);
                        if position_dirty&&last_position.elapsed()>=Duration::from_millis(500){let _=atomic_write(&config_dir().join("config.json"),&serde_json::to_vec_pretty(&engine.config).unwrap());position_dirty=false;}
                        if last_fullscreen.elapsed()>=Duration::from_secs(2){last_fullscreen=Instant::now();if let Some(app)=&app{
                            let f=engine.config.fullscreen_hide&&super::platform::fullscreen(app);if f!=fullscreen{fullscreen=f;super::apply_visibility(app,&state,fullscreen);}
                            if let Some(w)=app.get_webview_window("pet"){
                                super::restore_position(&w,&engine.config);
                                if !fullscreen && !engine.config.user_hidden {
                                    super::platform::restore_topmost(&w);
                                }
                            }
                        }}
                    }
                }
                let preview = state
                    .animation_preview
                    .lock()
                    .unwrap()
                    .snapshot()
                    .state
                    .is_some();
                let banter_view =
                    banter.refresh(&engine, preview, state.clock.now(), dispatch_banter);
                if banter_view.revision != last_banter {
                    last_banter = banter_view.revision;
                    banter_subscribers
                        .retain(|_, channel| channel.send(banter_view.clone()).is_ok());
                }
                let p = engine.snapshot();
                if p.revision != last_broadcast {
                    last_broadcast = p.revision;
                    state
                        .render_receipts
                        .lock()
                        .unwrap()
                        .published(p.revision, received.unwrap_or_else(Instant::now));
                    subscribers.retain(|id, c| {
                        if let Some(app) = &app {
                            if let Some(w) =
                                app.get_webview_window(id.split(':').next().unwrap_or(""))
                            {
                                if !w.is_visible().unwrap_or(false) {
                                    return true;
                                }
                            } else {
                                return false;
                            }
                        }
                        c.send(p.clone()).is_ok()
                    });
                    let mut trace = PipelineTrace::new("projection", "published");
                    trace.after = Some((&p).into());
                    trace.subscribers = Some(subscribers.len());
                    state.trace(trace);
                }
            }
        });
        let router = Router::new()
            .route("/v2/event", post(receive))
            .route("/v2/health", get(health))
            .route("/v2/diagnostics", get(diagnostic_report))
            .layer(DefaultBodyLimit::max(MAX_ENVELOPE_BYTES))
            .layer(middleware::from_fn_with_state(native.clone(), authorize))
            .with_state(native.clone());
        tauri::async_runtime::spawn(async move {
            if let Ok(listener) = tokio::net::TcpListener::from_std(listener) {
                let _ = axum::serve(listener, router).await;
            }
        });
        atomic_write(
            &native.runtime_path,
            &serde_json::to_vec(&native.runtime).unwrap(),
        )?;
        Ok(native)
    }
    pub fn cleanup(&self) {
        if fs::read(&self.runtime_path)
            .ok()
            .and_then(|b| serde_json::from_slice::<RuntimeFile>(&b).ok())
            .is_some_and(|r| r.instance_id == self.runtime.instance_id)
        {
            let _ = fs::remove_file(&self.runtime_path);
        }
        let _ = atomic_write(
            &config_dir().join("config.json"),
            &serde_json::to_vec_pretty(&*self.config.lock().unwrap()).unwrap(),
        );
    }
    async fn banter_call<T>(
        &self,
        message: impl FnOnce(oneshot::Sender<T>) -> Message,
    ) -> Result<T, String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(message(tx))
            .await
            .map_err(|_| "桌宠服务已停止")?;
        rx.await.map_err(|_| "桌宠服务已停止".into())
    }
    pub async fn banter_subscribe(
        &self,
        view: Webview,
        token: String,
        callback: JavaScriptChannelId,
    ) -> Result<BanterProjection, String> {
        self.banter_call(|tx| Message::BanterSubscribe(Box::new(view), token, callback, tx))
            .await
    }
    pub async fn banter_unsubscribe(&self, id: String) {
        let _ = self.tx.send(Message::BanterUnsubscribe(id)).await;
    }
    pub async fn banter_projection(&self) -> Result<BanterProjection, String> {
        self.banter_call(Message::BanterGet).await
    }
    pub async fn banter_status(&self) -> Result<BanterStatus, String> {
        self.banter_call(Message::BanterStatus).await
    }
    #[cfg(feature = "e2e")]
    pub async fn banter_fixture(&self, progress: VisibleProgress) -> Result<(), String> {
        self.banter_call(|tx| Message::BanterFixture(progress, tx))
            .await
    }
    pub async fn banter_save(
        &self,
        config: BanterConfig,
        key: Option<String>,
        delete: bool,
    ) -> Result<BanterStatus, String> {
        self.banter_call(|tx| Message::BanterSave(config, key, delete, tx))
            .await?
    }
    pub async fn banter_probe(
        &self,
        config: BanterConfig,
        key: Option<String>,
        preview: bool,
    ) -> Result<String, String> {
        self.banter_call(|tx| Message::BanterProbe(config, key, preview, tx))
            .await?
    }
    pub async fn projection(&self) -> Result<Projection, String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Get(tx))
            .await
            .map_err(|_| "核心已停止")?;
        rx.await.map_err(|_| "核心已停止".into())
    }
    pub async fn subscribe(
        &self,
        webview: Webview,
        token: String,
        callback: JavaScriptChannelId,
    ) -> Result<Projection, String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Subscribe(Box::new(webview), token, callback, tx))
            .await
            .map_err(|_| "核心已停止")?;
        rx.await.map_err(|_| "核心已停止".into())
    }
    pub async fn unsubscribe(&self, id: String) {
        let _ = self.tx.send(Message::Unsubscribe(id)).await;
    }
    pub async fn configure(&self, c: Config) -> Result<Config, String> {
        let verify = c.clone();
        let character =
            tauri::async_runtime::spawn_blocking(move || super::validate_config(&verify))
                .await
                .map_err(|e| e.to_string())??;
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Config(c, character, tx))
            .await
            .map_err(|_| "核心已停止")?;
        rx.await.map_err(|_| "核心已停止".to_string())?
    }
    pub async fn diagnostics(&self) -> Result<Vec<Diagnostic>, String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Diagnostics(tx))
            .await
            .map_err(|_| "核心已停止")?;
        rx.await.map_err(|_| "核心已停止".into())
    }
    pub async fn set_preview_enabled(&self, enabled: bool) -> Result<AnimationPreview, String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::PreviewEnabled(enabled, tx))
            .await
            .map_err(|_| "核心已停止")?;
        rx.await.map_err(|_| "核心已停止".to_string())?
    }
    pub async fn set_preview_state(
        &self,
        selected: Option<DisplayState>,
    ) -> Result<AnimationPreview, String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::PreviewState(selected, tx))
            .await
            .map_err(|_| "核心已停止")?;
        rx.await.map_err(|_| "核心已停止".to_string())?
    }
    pub async fn reset(&self) {
        let _ = self.tx.send(Message::Reset).await;
    }
    pub fn position(&self, p: Position) {
        let _ = self.tx.try_send(Message::Position(p));
    }
    #[cfg(feature = "e2e")]
    pub async fn advance_clock(&self, ms: u64) -> Result<Projection, String> {
        if ms > 3_600_000 {
            return Err("测试时钟步长过大".into());
        }
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::AdvanceClock(ms, tx))
            .await
            .map_err(|_| "核心已停止")?;
        rx.await.map_err(|_| "核心已停止".into())
    }
}
async fn authorize(
    State(s): State<Arc<Native>>,
    headers: HeaderMap,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if headers.contains_key("origin") {
        let mut trace = PipelineTrace::new("http-auth", "origin-rejected");
        trace.status = Some(403);
        s.trace(trace);
        return StatusCode::FORBIDDEN.into_response();
    }
    let supplied = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or("");
    if supplied.len() != s.runtime.token.len()
        || !bool::from(supplied.as_bytes().ct_eq(s.runtime.token.as_bytes()))
    {
        let mut trace = PipelineTrace::new("http-auth", "authentication-rejected");
        trace.status = Some(401);
        s.trace(trace);
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let allowed = {
        let mut r = s.rate.lock().unwrap();
        let now = Instant::now();
        r.1 = (r.1 + now.duration_since(r.0).as_secs_f64() * 200.0).min(200.0);
        r.0 = now;
        if r.1 >= 1.0 {
            r.1 -= 1.0;
            true
        } else {
            false
        }
    };
    if !allowed {
        s.lost.fetch_add(1, Ordering::SeqCst);
        let mut trace = PipelineTrace::new("http-entry", "rate-limited");
        trace.status = Some(429);
        s.trace(trace);
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let response = next.run(request).await;
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        s.lost.fetch_add(1, Ordering::SeqCst);
        let mut trace = PipelineTrace::new("http-entry", "envelope-too-large");
        trace.status = Some(413);
        s.trace(trace);
    }
    response
}
async fn receive(State(s): State<Arc<Native>>, body: Bytes) -> Response {
    let received = Instant::now();
    let e: EnvelopeV2 = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(_) => {
            s.lost.fetch_add(1, Ordering::SeqCst);
            let mut trace = PipelineTrace::new("http-receive", "envelope-json-invalid");
            trace.status = Some(400);
            trace.input = Some(diagnostics::InputDiagnostics::inspect(&body));
            s.trace(trace);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    if !e.validate() {
        s.lost.fetch_add(1, Ordering::SeqCst);
        let mut trace = PipelineTrace::event("http-receive", "envelope-invalid", &e);
        trace.status = Some(400);
        s.trace(trace);
        return StatusCode::BAD_REQUEST.into_response();
    }
    let size = body.len();
    let id = e.id.clone();
    let mut trace = PipelineTrace::event("http-receive", "received", &e);
    trace.input = Some(diagnostics::InputDiagnostics::inspect(e.payload.as_bytes()));
    s.trace(trace);
    if !enqueue(&s.tx, &s.bytes, &s.lost, e, size, received) {
        let mut trace = PipelineTrace::new("http-queue", "queue-unavailable");
        trace.event_id = Some(code_musume_core::adapter::short_hash(&id));
        trace.status = Some(429);
        s.trace(trace);
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"accepted":true,"id":id,"instanceId":s.runtime.instance_id})),
    )
        .into_response()
}
fn enqueue(
    tx: &mpsc::Sender<Message>,
    bytes: &AtomicUsize,
    lost: &AtomicU64,
    e: EnvelopeV2,
    size: usize,
    received: Instant,
) -> bool {
    if bytes
        .try_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
            v.checked_add(size).filter(|v| *v <= MAX_QUEUE_BYTES)
        })
        .is_err()
    {
        lost.fetch_add(1, Ordering::SeqCst);
        return false;
    }
    if tx.try_send(Message::Event(e, size, received)).is_err() {
        bytes.fetch_sub(size, Ordering::SeqCst);
        lost.fetch_add(1, Ordering::SeqCst);
        return false;
    }
    true
}
async fn health(State(s): State<Arc<Native>>) -> Response {
    match s.projection().await {
        Ok(p) => {
            let fps = s
                .render_fps
                .lock()
                .unwrap()
                .filter(|(at, _)| at.elapsed() < Duration::from_secs(5))
                .map(|(_, fps)| fps);
            let receipt = s.render_receipts.lock().unwrap().latest.clone();
            Json(serde_json::json!({"v":2,"appVersion":s.runtime.app_version,"projection":p,"renderFps":fps,"renderReceipt":receipt})).into_response()
        }
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}
async fn diagnostic_report(State(s): State<Arc<Native>>) -> Response {
    match s.diagnostic_report().await {
        Ok(report) => Json(report).into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> EnvelopeV2 {
        EnvelopeV2 {
            v: 2,
            id: "fixture".into(),
            agent: Agent::Codex,
            event: "Stop".into(),
            ts: 0,
            src: EnvelopeSrc {
                cli: "test".into(),
                pid: 0,
            },
            payload: "{}".into(),
            parse_error: None,
        }
    }
    #[test]
    fn full_entry_queue_rolls_back_bytes_and_records_loss() {
        let (tx, mut rx) = mpsc::channel(MAX_QUEUE_ENTRIES);
        let bytes = AtomicUsize::new(0);
        let lost = AtomicU64::new(0);
        for _ in 0..MAX_QUEUE_ENTRIES {
            assert!(enqueue(&tx, &bytes, &lost, event(), 10, Instant::now()));
        }
        assert!(!enqueue(&tx, &bytes, &lost, event(), 10, Instant::now()));
        assert_eq!(bytes.load(Ordering::SeqCst), MAX_QUEUE_ENTRIES * 10);
        assert_eq!(lost.load(Ordering::SeqCst), 1);
        assert!(rx.try_recv().is_ok());
        bytes.fetch_sub(10, Ordering::SeqCst);
        assert!(enqueue(&tx, &bytes, &lost, event(), 10, Instant::now()));
    }
    #[test]
    fn byte_limit_is_inclusive_and_rejection_keeps_accounting() {
        let (tx, _rx) = mpsc::channel(MAX_QUEUE_ENTRIES);
        let bytes = AtomicUsize::new(0);
        let lost = AtomicU64::new(0);
        for _ in 0..4 {
            assert!(enqueue(
                &tx,
                &bytes,
                &lost,
                event(),
                MAX_ENVELOPE_BYTES,
                Instant::now()
            ));
        }
        assert_eq!(bytes.load(Ordering::SeqCst), MAX_QUEUE_BYTES);
        assert!(!enqueue(&tx, &bytes, &lost, event(), 1, Instant::now()));
        assert_eq!(bytes.load(Ordering::SeqCst), MAX_QUEUE_BYTES);
        assert_eq!(lost.load(Ordering::SeqCst), 1);
    }
}
