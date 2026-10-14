use code_musume_contracts::RuntimeFile;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
pub fn data_dir() -> PathBuf {
    env::var_os("CODE_MUSUME_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(env::temp_dir)
                .join("code-musume")
        })
}
pub fn config_dir() -> PathBuf {
    env::var_os("CODE_MUSUME_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("APPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(env::temp_dir)
                .join("code-musume")
        })
}
pub fn runtime_path(mode: &str) -> PathBuf {
    data_dir().join(if mode == "dev" {
        "runtime.dev.json"
    } else {
        "runtime.json"
    })
}
pub fn read_runtime(path: &Path) -> Result<RuntimeFile, String> {
    let bytes = fs::read(path).map_err(|_| "桌宠尚未运行".to_string())?;
    if bytes.len() > 8192 {
        return Err("运行信息损坏".into());
    }
    let r: RuntimeFile = serde_json::from_slice(&bytes).map_err(|_| "运行信息损坏".to_string())?;
    if r.v != 2
        || r.port == 0
        || r.token.len() != 64
        || !r.token.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err("运行信息版本或格式不匹配".into());
    }
    Ok(r)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    let result = fs::rename(&tmp, path).map_err(|e| e.to_string());
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
