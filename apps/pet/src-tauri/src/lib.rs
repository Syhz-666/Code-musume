use tauri::Manager;

/// 应用装配（Phase 1 骨架）。
/// 顺序有意义：single-instance 必须**最先**注册（官方要求，P11）。
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 第二次启动：显示已有实例的桌宠窗口
            if let Some(w) = app.get_webview_window("pet") {
                let _ = w.show();
            }
        }))
        .invoke_handler(tauri::generate_handler![pet_ready])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 前端首帧渲染完成后调用 → 显示窗口（消灭白闪，架构 §7.1）。
#[tauri::command]
fn pet_ready(app: tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("pet") {
        let _ = w.show();
    }
}
