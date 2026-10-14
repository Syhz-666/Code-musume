mod characters;
mod platform;
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
fn open_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
    } else {
        let _ = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
            .title("代码娘 · 设置")
            .inner_size(850.0, 760.0)
            .min_inner_size(640.0, 500.0)
            .build();
    }
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
async fn get_diagnostics(s: State<'_, Arc<Native>>) -> Result<Vec<Diagnostic>, String> {
    s.diagnostics().await
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
fn show_settings(app: AppHandle) {
    open_settings(&app);
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
async fn get_character_asset(id: String, form: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || characters::asset(&id, &form))
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
    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("代码娘")
        .show_menu_on_left_click(true);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .on_menu_event(|app, event| match event.id.as_ref() {
            "settings" => open_settings(app),
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
        })
        .build(app)?;
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
    let app = tauri::Builder::default()
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
            subscribe_projection,
            unsubscribe_projection,
            get_projection,
            get_config,
            set_config,
            get_diagnostics,
            reset_sessions,
            list_characters,
            get_character,
            get_character_asset,
            import_character,
            choose_path,
            report_render_stats,
            preview_hooks,
            apply_hooks,
            hook_status
        ])
        .build(tauri::generate_context!())
        .expect("build code-musume");
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            if let Some(s) = app.try_state::<Arc<Native>>() {
                s.cleanup();
            }
        }
    });
}
