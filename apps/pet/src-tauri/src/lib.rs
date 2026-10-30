mod animation_preview;
mod characters;
#[cfg(feature = "e2e")]
mod e2e_environment;
mod pipeline_diagnostics;
mod platform;
mod render_receipts;
mod service;
use code_musume_contracts::*;
use code_musume_hook::{installer, runtime};
use service::Native;
use std::{path::PathBuf, sync::Arc};
use tauri::{
    ipc::Channel,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

struct DesktopMenu(Menu<tauri::Wry>);

fn mode() -> &'static str {
    if cfg!(debug_assertions) {
        "dev"
    } else {
        "release"
    }
}
fn validate_config(c: &Config) -> Result<CharacterManifest, String> {
    if c.schema_version != 1
        || !matches!(c.renderer.as_str(), "pixi" | "dom")
        || c.agent_forms.len() > 8
    {
        return Err("设置格式无效".into());
    }
    let m = characters::load(&c.character_id)?;
    if c.form_lock
        .as_ref()
        .is_some_and(|id| !m.forms.iter().any(|f| &f.id == id))
        || c.agent_forms
            .values()
            .any(|id| !m.forms.iter().any(|f| &f.id == id))
    {
        return Err("所选形态不属于当前角色".into());
    }
    Ok(m)
}
fn open_settings(app: &AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("settings") {
        w.show().map_err(|e| e.to_string())?;
        w.set_focus().map_err(|e| e.to_string())?;
    } else {
        let builder =
            WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
                .title("代码娘 · 设置")
                .inner_size(850.0, 760.0)
                .min_inner_size(640.0, 500.0);
        #[cfg(feature = "e2e")]
        let builder = e2e_environment::configure_window(builder, app)?;
        builder.build().map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn apply_visibility(app: &AppHandle, s: &Native, fullscreen: bool) {
    let c = s.config.lock().unwrap().clone();
    if let Some(w) = app.get_webview_window("pet") {
        let visible = !c.user_hidden && !fullscreen;
        if visible {
            let _ = w.show();
            let _ = w.set_ignore_cursor_events(c.ghost);
        } else {
            let _ = w.hide();
        }
        let _ = app.emit_to("pet", "pet-visibility", visible);
    }
}
fn restore_position(w: &WebviewWindow, c: &Config) {
    if let Ok(monitors) = w.available_monitors() {
        let monitor = c
            .position
            .as_ref()
            .and_then(|p| {
                monitors
                    .iter()
                    .find(|m| m.name().map(|s| s == &p.monitor_id).unwrap_or(false))
            })
            .or_else(|| monitors.first());
        if let Some(m) = monitor {
            let size = w.outer_size().unwrap_or(tauri::PhysicalSize::new(300, 350));
            let area = m.work_area();
            let pos = area.position;
            let ms = area.size;
            let default = (
                pos.x + ms.width as i32 - size.width as i32 - 40,
                pos.y + ms.height as i32 - size.height as i32 - 80,
            );
            let (x, y) = c
                .position
                .as_ref()
                .map_or(default, |p| (p.x_phys, p.y_phys));
            let maxx = (pos.x + ms.width as i32 - size.width as i32).max(pos.x);
            let maxy = (pos.y + ms.height as i32 - size.height as i32).max(pos.y);
            let next = tauri::PhysicalPosition::new(x.clamp(pos.x, maxx), y.clamp(pos.y, maxy));
            if w.outer_position().ok() != Some(next) {
                let _ = w.set_position(next);
            }
        }
    }
}
fn hook_options(
    agent: Agent,
    scope: &str,
    cwd: Option<String>,
) -> Result<installer::Options, String> {
    let root = if scope == "user" {
        PathBuf::from(std::env::var_os("USERPROFILE").ok_or("找不到用户目录")?)
    } else if scope == "project" {
        let p = PathBuf::from(cwd.ok_or("请填写项目目录")?);
        if !p.is_absolute() || !p.is_dir() {
            return Err("项目目录必须是已存在的绝对路径".into());
        }
        p
    } else {
        return Err("无效范围".into());
    };
    let exe = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("code-musume-hook.exe");
    Ok(installer::Options {
        agent,
        root,
        exe,
        runtime: mode().into(),
        state_dir: runtime::data_dir(),
    })
}
#[tauri::command]
async fn subscribe_projection(
    window: WebviewWindow,
    s: State<'_, Arc<Native>>,
    on_update: Channel<Projection>,
    subscription_id: String,
) -> Result<Projection, String> {
    s.subscribe(format!("{}:{subscription_id}", window.label()), on_update)
        .await
}
#[tauri::command]
async fn unsubscribe_projection(
    window: WebviewWindow,
    s: State<'_, Arc<Native>>,
    subscription_id: String,
) -> Result<(), String> {
    s.unsubscribe(format!("{}:{subscription_id}", window.label()))
        .await;
    Ok(())
}
#[tauri::command]
async fn get_projection(s: State<'_, Arc<Native>>) -> Result<Projection, String> {
    s.projection().await
}
#[tauri::command]
fn get_config(s: State<'_, Arc<Native>>) -> Config {
    s.config.lock().unwrap().clone()
}
#[tauri::command]
async fn set_config(config: Config, s: State<'_, Arc<Native>>) -> Result<Config, String> {
    s.configure(config).await
}
#[tauri::command]
fn get_animation_preview(s: State<'_, Arc<Native>>) -> AnimationPreview {
    s.animation_preview.lock().unwrap().snapshot()
}
#[tauri::command]
async fn set_animation_preview_enabled(
    enabled: bool,
    s: State<'_, Arc<Native>>,
) -> Result<AnimationPreview, String> {
    s.set_preview_enabled(enabled).await
}
#[tauri::command]
async fn set_animation_preview_state(
    state: Option<DisplayState>,
    s: State<'_, Arc<Native>>,
) -> Result<AnimationPreview, String> {
    s.set_preview_state(state).await
}
#[tauri::command]
async fn get_diagnostics(s: State<'_, Arc<Native>>) -> Result<Vec<Diagnostic>, String> {
    s.diagnostics().await
}
#[tauri::command]
async fn get_pipeline_diagnostics(s: State<'_, Arc<Native>>) -> Result<serde_json::Value, String> {
    s.diagnostic_report().await
}
#[tauri::command]
async fn reset_sessions(s: State<'_, Arc<Native>>) -> Result<(), String> {
    s.reset().await;
    Ok(())
}
#[tauri::command]
fn pet_ready(app: AppHandle, s: State<'_, Arc<Native>>) {
    let hide = s.config.lock().unwrap().fullscreen_hide;
    apply_visibility(&app, &s, platform::fullscreen(&app) && hide);
}
#[tauri::command]
async fn show_settings(app: AppHandle) -> Result<(), String> {
    open_settings(&app)
}
#[tauri::command]
async fn show_pet_menu(
    window: WebviewWindow,
    menu: State<'_, DesktopMenu>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    if window.label() != "pet" || !x.is_finite() || !y.is_finite() {
        return Err("无效的桌宠菜单位置".into());
    }
    // CSS client coordinates are logical pixels; Windows places the native menu
    // within the monitor work area even when the pet is against a screen edge.
    window
        .popup_menu_at(&menu.0, tauri::LogicalPosition::new(x, y))
        .map_err(|e| e.to_string())
}
#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}
#[tauri::command]
async fn list_characters() -> Result<Vec<CharacterManifest>, String> {
    tauri::async_runtime::spawn_blocking(characters::list)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn get_character(id: String) -> Result<CharacterManifest, String> {
    tauri::async_runtime::spawn_blocking(move || characters::load(&id))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn get_character_asset(
    id: String,
    form: String,
    image: Option<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        characters::asset_page(&id, &form, image.as_deref())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn import_character(source: String) -> Result<CharacterManifest, String> {
    tauri::async_runtime::spawn_blocking(move || characters::import(&PathBuf::from(source)))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn choose_path(app: AppHandle, folder: bool) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        let picker = app.dialog().file();
        let chosen = if folder {
            picker.blocking_pick_folder()
        } else {
            picker
                .add_filter("角色图集", &["png", "webp"])
                .blocking_pick_file()
        };
        chosen
            .map(|p| {
                p.into_path()
                    .map(|p| p.to_string_lossy().into_owned())
                    .map_err(|e| e.to_string())
            })
            .transpose()
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn report_render_stats(s: State<'_, Arc<Native>>, fps: f64) {
    if fps.is_finite() && (0.0..=240.0).contains(&fps) {
        *s.render_fps.lock().unwrap() = Some((std::time::Instant::now(), fps));
    }
}
#[tauri::command]
fn report_rendered(
    window: WebviewWindow,
    s: State<'_, Arc<Native>>,
    instance_id: String,
    revision: u64,
) -> bool {
    let preview = s.animation_preview.lock().unwrap();
    let accepted = window.label() == "pet"
        && !preview.active()
        && s.render_receipts.lock().unwrap().acknowledge(
            &s.runtime.instance_id,
            &instance_id,
            revision,
            std::time::Instant::now(),
        );
    let mut trace = pipeline_diagnostics::PipelineTrace::new(
        "render",
        if accepted {
            "frame-submitted"
        } else if preview.active() {
            "manual-preview-active"
        } else {
            "receipt-not-current"
        },
    );
    trace.revision = Some(revision);
    s.trace(trace);
    accepted
}
#[cfg(feature = "e2e")]
#[tauri::command]
async fn e2e_advance_clock(s: State<'_, Arc<Native>>, ms: u64) -> Result<Projection, String> {
    s.advance_clock(ms).await
}
#[cfg(feature = "e2e")]
#[tauri::command]
async fn e2e_recreate_pet(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window("pet") {
        w.destroy().map_err(|e| e.to_string())?;
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while app.get_webview_window("pet").is_some() {
        if std::time::Instant::now() > deadline {
            return Err("测试窗口尚未完成销毁".into());
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    // Keep the settings window alive and reconstruct the production window configuration.
    let config = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == "pet")
        .ok_or("missing pet config")?;
    WebviewWindowBuilder::from_config(&app, config)
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[cfg(feature = "e2e")]
#[tauri::command]
fn e2e_exit(app: AppHandle) {
    app.exit(0);
}
#[tauri::command]
fn preview_hooks(
    agent: Agent,
    operation: String,
    scope: String,
    cwd: Option<String>,
) -> Result<InstallPreview, String> {
    installer::preview(&hook_options(agent, &scope, cwd)?, &operation)
}
#[tauri::command]
fn apply_hooks(
    agent: Agent,
    operation: String,
    scope: String,
    cwd: Option<String>,
    digest: String,
) -> Result<InstallPreview, String> {
    installer::apply(&hook_options(agent, &scope, cwd)?, &operation, &digest)
}
#[tauri::command]
fn hook_status(agent: Agent, scope: String, cwd: Option<String>) -> Result<HookStatus, String> {
    installer::status(&hook_options(agent, &scope, cwd)?)
}
fn tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "show", "显示", true, None::<&str>)?,
            &MenuItem::with_id(app, "hide", "隐藏", true, None::<&str>)?,
            &MenuItem::with_id(app, "ghost", "切换点击穿透", true, None::<&str>)?,
            &MenuItem::with_id(app, "settings", "设置与接入", true, None::<&str>)?,
            &MenuItem::with_id(app, "exit", "退出", true, None::<&str>)?,
        ],
    )?;
    let icon = image::load_from_memory_with_format(
        include_bytes!("../icons/tray-icon.png"),
        image::ImageFormat::Png,
    )?
    .into_rgba8();
    let (width, height) = icon.dimensions();
    let builder = TrayIconBuilder::new()
        .icon(tauri::image::Image::new_owned(
            icon.into_raw(),
            width,
            height,
        ))
        .icon_as_template(false)
        .menu(&menu)
        .tooltip("代码娘")
        .show_menu_on_left_click(true);
    app.manage(DesktopMenu(menu.clone()));
    // Both the pet context menu and the tray use the same entries and handler.
    app.on_menu_event(|app, event| match event.id.as_ref() {
        "settings" => {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let _ = open_settings(&app);
            });
        }
        "exit" => app.exit(0),
        id => {
            let native = app.state::<Arc<Native>>().inner().clone();
            let mut c = native.config.lock().unwrap().clone();
            match id {
                "show" => c.user_hidden = false,
                "hide" => c.user_hidden = true,
                "ghost" => c.ghost = !c.ghost,
                _ => return,
            }
            tauri::async_runtime::spawn(async move {
                let _ = native.configure(c).await;
            });
        }
    });
    builder.build(app)?;
    Ok(())
}
pub fn run() {
    if std::env::args().any(|a| a == "--headless") {
        let native = Native::start(None, mode()).expect("start headless core");
        let seconds = std::env::var("CODE_MUSUME_HEADLESS_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(60);
        std::thread::sleep(std::time::Duration::from_secs(seconds));
        native.cleanup();
        return;
    }
    let builder = tauri::Builder::default();
    let context = tauri::generate_context!();
    #[cfg(feature = "e2e")]
    let context =
        e2e_environment::configure_context(context).expect("configure isolated test WebViews");
    #[cfg(feature = "e2e")]
    let builder = builder
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());
    let app = builder
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(s) = app.try_state::<Arc<Native>>() {
                let mut c = s.config.lock().unwrap().clone();
                c.user_hidden = false;
                let native = s.inner().clone();
                tauri::async_runtime::spawn(async move {
                    let _ = native.configure(c).await;
                });
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let native =
                Native::start(Some(app.handle().clone()), mode()).map_err(std::io::Error::other)?;
            if let Some(w) = app.get_webview_window("pet") {
                restore_position(&w, &native.config.lock().unwrap());
            }
            app.manage(native);
            tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|w, event| {
            if w.label() == "pet" {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        if let Some(s) = w.app_handle().try_state::<Arc<Native>>() {
                            let mut c = s.config.lock().unwrap().clone();
                            c.user_hidden = true;
                            let native = s.inner().clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = native.configure(c).await;
                            });
                        }
                    }
                    tauri::WindowEvent::Moved(p) => {
                        if let Some(s) = w.app_handle().try_state::<Arc<Native>>() {
                            let id = w
                                .current_monitor()
                                .ok()
                                .flatten()
                                .and_then(|m| m.name().cloned())
                                .unwrap_or("primary".into());
                            s.position(Position {
                                monitor_id: id,
                                x_phys: p.x,
                                y_phys: p.y,
                            });
                        }
                    }
                    _ => {}
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            pet_ready,
            show_settings,
            show_pet_menu,
            quit_app,
            subscribe_projection,
            unsubscribe_projection,
            get_projection,
            get_config,
            set_config,
            get_animation_preview,
            set_animation_preview_enabled,
            set_animation_preview_state,
            get_diagnostics,
            get_pipeline_diagnostics,
            reset_sessions,
            list_characters,
            get_character,
            get_character_asset,
            import_character,
            choose_path,
            report_render_stats,
            report_rendered,
            preview_hooks,
            apply_hooks,
            hook_status,
            #[cfg(feature = "e2e")]
            e2e_advance_clock,
            #[cfg(feature = "e2e")]
            e2e_recreate_pet,
            #[cfg(feature = "e2e")]
            e2e_exit
        ])
        .build(context)
        .expect("build code-musume");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Some(s) = app.try_state::<Arc<Native>>() {
                s.cleanup();
            }
        }
    });
}
