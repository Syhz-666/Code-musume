//! Test-only WebView options. Elevated Windows hosts ignore WEBVIEW2_* env overrides.
use std::path::PathBuf;
use tauri::{AppHandle, Context, Manager, WebviewWindowBuilder, Wry};

pub fn configure_context(mut context: Context<Wry>) -> Result<Context<Wry>, String> {
    let directory = PathBuf::from(
        std::env::var_os("CODE_MUSUME_UI_RUN_DIR").ok_or("Missing isolated UI run directory")?,
    );
    if !directory.is_absolute() {
        return Err("UI run directory must be absolute".into());
    }
    let motion =
        std::env::var("CODE_MUSUME_UI_SYSTEM_MOTION").unwrap_or_else(|_| "no-preference".into());
    let motion_flag = match motion.as_str() {
        "reduce" => "--force-prefers-reduced-motion",
        "no-preference" => "--force-prefers-no-reduced-motion",
        _ => return Err("Unsupported UI system motion preference".into()),
    };
    let arguments = format!(
        "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
         --force-device-scale-factor=1 --force-color-profile=srgb --disable-lcd-text {motion_flag}"
    );
    for window in &mut context.config_mut().app.windows {
        window.additional_browser_args = Some(arguments.clone());
        window.data_directory = Some(directory.join("webview"));
    }
    eprintln!(
        "[e2e-environment] native WebView options: motion={motion}; dpr=1; isolatedData=true"
    );
    Ok(context)
}

pub fn configure_window<'a, M: Manager<Wry>>(
    builder: WebviewWindowBuilder<'a, Wry, M>,
    app: &AppHandle,
) -> Result<WebviewWindowBuilder<'a, Wry, M>, String> {
    // The initial pet, settings and reconstructed pet must share the same options.
    let config = app.config();
    let pet = config
        .app
        .windows
        .iter()
        .find(|w| w.label == "pet")
        .ok_or("Missing test pet window configuration")?;
    Ok(builder
        .additional_browser_args(
            pet.additional_browser_args
                .as_deref()
                .ok_or("Missing native test browser arguments")?,
        )
        .data_directory(
            pet.data_directory
                .clone()
                .ok_or("Missing test WebView directory")?,
        ))
}
