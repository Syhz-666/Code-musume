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
use code_musume_core::Engine;
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
use tauri::{ipc::Channel, AppHandle, Manager};
use tokio::sync::{mpsc, oneshot};

enum Message {
    Event(EnvelopeV2, usize, Instant),
    Subscribe(String, Channel<Projection>, oneshot::Sender<Projection>),
    Unsubscribe(String),
    Get(oneshot::Sender<Projection>),
    Config(
        Config,
        CharacterManifest,
        oneshot::Sender<Result<Config, String>>,
    ),
    Diagnostics(oneshot::Sender<Vec<Diagnostic>>),
    Reset,
    Position(Position),
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
    clock: Clock,
}
impl Native {
    pub fn start(app: Option<AppHandle>, mode: &str) -> Result<Arc<Self>, String> {
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
        let native = Arc::new(Self {
            clock: Clock {
                start: Instant::now(),
                #[cfg(feature = "e2e")]
                offset: AtomicU64::new(0),
            },
            render_receipts: Mutex::new(super::render_receipts::RenderReceipts::default()),
            tx,
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
        tauri::async_runtime::spawn(async move {
            let mut engine = Engine::new(instance, config);
            engine.character_forms = character.agent_forms;
            engine.default_form = character.default_form;
            let mut subscribers: BTreeMap<String, Channel<Projection>> = BTreeMap::new();
            let mut last_broadcast = 0;
            let mut timer = tokio::time::interval(Duration::from_millis(250));
            let mut position_dirty = false;
            let mut last_position = Instant::now();
            let mut fullscreen = false;
            let mut last_fullscreen = Instant::now();
            loop {
                let now = state.clock.now();
                let mut received = None;
                tokio::select! {
                    msg=rx.recv()=>match msg{
                        Some(Message::Event(e,size,at))=>{received=Some(at);state.bytes.fetch_sub(size,Ordering::SeqCst);engine.ingest(e,state.clock.now());},
                        Some(Message::Subscribe(id,c,reply))=>{let label=id.split(':').next().unwrap_or("");subscribers.retain(|old,_|old.split(':').next()!=Some(label));let p=engine.snapshot();let _=c.send(p.clone());subscribers.insert(id,c);let _=reply.send(p);},
                        Some(Message::Unsubscribe(id))=>{subscribers.remove(&id);},
                        Some(Message::Get(reply))=>{engine.tick(state.clock.now());let _=reply.send(engine.snapshot());},
                        #[cfg(feature = "e2e")]
                        Some(Message::AdvanceClock(ms,reply))=>{state.clock.offset.fetch_add(ms,Ordering::SeqCst);engine.tick(state.clock.now());let _=reply.send(engine.snapshot());},
                        Some(Message::Diagnostics(reply))=>{let _=reply.send(engine.diagnostics());},
                        Some(Message::Reset)=>engine.reset(now),
                        Some(Message::Position(p))=>{engine.config.position=Some(p);*state.config.lock().unwrap()=engine.config.clone();position_dirty=true;last_position=Instant::now();},
                        Some(Message::Config(mut c,character,reply))=>{
                            c.position=engine.config.position.clone();
                            let result=(||{
                                if c.autostart!=engine.config.autostart{super::platform::autostart(c.autostart)?;}
                                if let Err(e)=atomic_write(&config_dir().join("config.json"),&serde_json::to_vec_pretty(&c).unwrap()){
                                    if c.autostart!=engine.config.autostart{let _=super::platform::autostart(engine.config.autostart);}return Err(e);
                                }Ok(c)
                            })();
                            if let Ok(c)=&result{engine.config=c.clone();engine.character_forms=character.agent_forms;engine.default_form=character.default_form;*state.config.lock().unwrap()=c.clone();engine.tick(now);if let Some(app)=&app{fullscreen=c.fullscreen_hide&&super::platform::fullscreen(app);super::apply_visibility(app,&state,fullscreen);}}
                            let _=reply.send(result);
                        },
                        None=>break,
                    },
                    _=timer.tick()=>{
                        let now=state.clock.now();let lost=state.lost.swap(0,Ordering::SeqCst);if lost>0{engine.lost(lost,now);}engine.tick(now);
                        if position_dirty&&last_position.elapsed()>=Duration::from_millis(500){let _=atomic_write(&config_dir().join("config.json"),&serde_json::to_vec_pretty(&engine.config).unwrap());position_dirty=false;}
                        if last_fullscreen.elapsed()>=Duration::from_secs(2){last_fullscreen=Instant::now();if let Some(app)=&app{
                            let f=engine.config.fullscreen_hide&&super::platform::fullscreen(app);if f!=fullscreen{fullscreen=f;super::apply_visibility(app,&state,fullscreen);}
                            if let Some(w)=app.get_webview_window("pet"){super::restore_position(&w,&engine.config);}
                        }}
                    }
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
                }
            }
        });
        let router = Router::new()
            .route("/v2/event", post(receive))
            .route("/v2/health", get(health))
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
        id: String,
        c: Channel<Projection>,
    ) -> Result<Projection, String> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(Message::Subscribe(id, c, tx))
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
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    let response = next.run(request).await;
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        s.lost.fetch_add(1, Ordering::SeqCst);
    }
    response
}
async fn receive(State(s): State<Arc<Native>>, body: Bytes) -> Response {
    let received = Instant::now();
    let e: EnvelopeV2 = match serde_json::from_slice(&body) {
        Ok(e) => e,
        Err(_) => {
            s.lost.fetch_add(1, Ordering::SeqCst);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };
    if !e.validate() {
        s.lost.fetch_add(1, Ordering::SeqCst);
        return StatusCode::BAD_REQUEST.into_response();
    }
    let size = body.len();
    let id = e.id.clone();
    if !enqueue(&s.tx, &s.bytes, &s.lost, e, size, received) {
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
