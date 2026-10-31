//! Reversible configuration edits. Trust stores are never touched.
use crate::runtime::atomic_write;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use code_musume_contracts::{Agent, HookStatus, InstallPreview};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CLAUDE: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "PermissionRequest",
    "PermissionDenied",
    "Notification",
    "Stop",
    "StopFailure",
    "SubagentStart",
    "SubagentStop",
    "PreCompact",
    "PostCompact",
];
const CODEX: &[&str] = &[
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Stop",
    "Interrupt",
    "SubagentStart",
    "SubagentStop",
    "PreCompact",
    "PostCompact",
];
#[derive(Clone)]
pub struct Options {
    pub agent: Agent,
    pub root: PathBuf,
    pub exe: PathBuf,
    pub runtime: String,
    pub state_dir: PathBuf,
}
#[derive(Serialize, Deserialize)]
struct Receipt {
    original: Vec<u8>,
    installed_digest: String,
    #[serde(default)]
    target: PathBuf,
    #[serde(default)]
    root: PathBuf,
    #[serde(default)]
    agent: Option<Agent>,
    #[serde(default)]
    runtime: String,
}
fn digest(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}
pub fn handler(agent: Agent, event: &str, exe: &Path, runtime: &str) -> Value {
    let args = [
        "--agent",
        agent.as_str(),
        "--event",
        event,
        "--runtime",
        runtime,
    ];
    if agent == Agent::ClaudeCode {
        return json!({"type":"command","command":exe.to_string_lossy(),"args":args,"async":true,"timeout":5});
    }
    let mut script = format!(
        "try {{ & {} {}",
        literal(&exe.to_string_lossy()),
        args.iter()
            .map(|a| literal(a))
            .collect::<Vec<_>>()
            .join(" ")
    );
    if event == "SessionEnd" {
        script.push_str(" '--budget-ms' '100'");
    }
    script.push_str(&format!(
        " 1>$null 2>$null }} catch {{}}; exit 0 # code-musume-hook:codex:{runtime}"
    ));
    let bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let command = format!(
        "powershell.exe -NoProfile -NonInteractive -EncodedCommand {}",
        STANDARD.encode(bytes)
    );
    json!({"type":"command","command":command,"commandWindows":command,"async":event != "SessionEnd","timeout":if matches!(event,"SessionEnd"|"Interrupt") {1} else {5}})
}
fn owned(v: &Value, o: &Options) -> bool {
    if o.agent == Agent::ClaudeCode {
        let stem = v["command"]
            .as_str()
            .and_then(|s| Path::new(s).file_stem())
            .and_then(|s| s.to_str());
        let a = v["args"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default();
        stem == Some("code-musume-hook")
            && a.windows(2).any(|p| p == ["--agent", o.agent.as_str()])
            && a.windows(2).any(|p| p == ["--runtime", o.runtime.as_str()])
    } else {
        let Some(c) = v["commandWindows"]
            .as_str()
            .or_else(|| v["command"].as_str())
        else {
            return false;
        };
        let Some(encoded) =
            c.strip_prefix("powershell.exe -NoProfile -NonInteractive -EncodedCommand ")
        else {
            return false;
        };
        let Ok(b) = STANDARD.decode(encoded) else {
            return false;
        };
        let u: Vec<u16> = b
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        String::from_utf16(&u)
            .is_ok_and(|s| s.ends_with(&format!("; exit 0 # code-musume-hook:codex:{}", o.runtime)))
    }
}
fn target(o: &Options) -> Result<PathBuf, String> {
    if o.agent == Agent::ClaudeCode {
        return Ok(o.root.join(".claude/settings.json"));
    }
    let p = o.root.join(".codex/config.toml");
    if p.exists() {
        let text = fs::read_to_string(&p).map_err(|e| e.to_string())?;
        let v: Value = toml_edit::de::from_str(&text).map_err(|_| "Codex TOML 损坏，拒绝写入")?;
        // Codex 也把信任/停用记录写在 `[hooks.state]` 下；只有事件数组才算内联接线。
        if v.get("hooks")
            .and_then(Value::as_object)
            .is_some_and(|h| h.values().any(|v| v.is_array()))
        {
            return Ok(p);
        }
    }
    Ok(o.root.join(".codex/hooks.json"))
}
fn load(p: &Path) -> Result<(Vec<u8>, Value), String> {
    let b = match fs::read(p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e.to_string()),
    };
    if b.len() > 2 * 1024 * 1024 {
        return Err("配置过大，拒绝写入".into());
    }
    if b.is_empty() && !p.exists() {
        return Ok((b, json!({})));
    }
    let v: Value = if p.extension().is_some_and(|s| s == "toml") {
        toml_edit::de::from_str(std::str::from_utf8(&b).map_err(|_| "配置编码无效")?)
            .map_err(|_| "配置 TOML 损坏")?
    } else {
        serde_json::from_slice(&b).map_err(|_| "配置 JSON 损坏，拒绝写入（首版要求严格 JSON）")?
    };
    if !v.is_object() {
        return Err("配置必须为对象".into());
    }
    Ok((b, v))
}
fn transform(mut v: Value, o: &Options, install: bool) -> Result<Value, String> {
    if v.get("hooks").is_none() {
        if !install {
            return Ok(v);
        }
        v["hooks"] = json!({});
    }
    let hooks = v["hooks"].as_object_mut().ok_or("hooks 必须为对象")?;
    let mut removed_events = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        let a = groups.as_array_mut().ok_or("hook 分组必须为数组")?;
        let mut removed_groups = Vec::new();
        for (i, g) in a.iter_mut().enumerate() {
            let h = g
                .get_mut("hooks")
                .and_then(Value::as_array_mut)
                .ok_or("hook handlers 必须为数组")?;
            let before = h.len();
            h.retain(|h| !owned(h, o));
            if before > 0 && h.is_empty() {
                removed_groups.push(i);
            }
        }
        let changed = !removed_groups.is_empty();
        for i in removed_groups.into_iter().rev() {
            a.remove(i);
        }
        if changed && a.is_empty() {
            removed_events.push(event.clone());
        }
    }
    for event in removed_events {
        hooks.remove(&event);
    }
    if install {
        for event in if o.agent == Agent::ClaudeCode {
            CLAUDE
        } else {
            CODEX
        } {
            hooks
                .entry((*event).to_owned())
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .ok_or("hook 分组必须为数组")?
                .push(json!({"hooks":[handler(o.agent,event,&o.exe,&o.runtime)]}));
        }
    }
    if hooks.is_empty() {
        v.as_object_mut().unwrap().remove("hooks");
    }
    Ok(v)
}
fn encode(p: &Path, b: &[u8], v: &Value) -> Result<Vec<u8>, String> {
    if p.extension().is_some_and(|s| s == "toml") {
        let mut doc = std::str::from_utf8(b)
            .map_err(|_| "配置编码无效")?
            .parse::<toml_edit::DocumentMut>()
            .map_err(|e| e.to_string())?;
        if let Some(h) = v.get("hooks") {
            let fresh =
                toml_edit::ser::to_document(&json!({"hooks":h})).map_err(|e| e.to_string())?;
            doc["hooks"] = fresh["hooks"].clone();
        } else {
            doc.remove("hooks");
        }
        Ok(doc.to_string().into_bytes())
    } else {
        let mut out = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
        out.push(b'\n');
        Ok(out)
    }
}
fn receipt(o: &Options, p: &Path) -> PathBuf {
    o.state_dir.join("installs").join(format!(
        "{}-{}-{}.json",
        o.agent.as_str(),
        o.runtime,
        digest(p.to_string_lossy().as_bytes())
    ))
}
fn build(o: &Options, op: &str) -> Result<(InstallPreview, Vec<u8>, Vec<u8>), String> {
    if !matches!(op, "install" | "uninstall") {
        return Err("无效安装操作".into());
    }
    if op == "install" && !o.exe.is_file() {
        return Err("hook 可执行文件不存在，请先构建 CLI".into());
    }
    let path = target(o)?;
    let (before, v) = load(&path)?;
    let after_v = transform(v.clone(), o, op == "install")?;
    let mut after = if v == after_v {
        before.clone()
    } else {
        encode(&path, &before, &after_v)?
    };
    if op == "uninstall" {
        if let Some(r) = fs::read(receipt(o, &path))
            .ok()
            .and_then(|b| serde_json::from_slice::<Receipt>(&b).ok())
        {
            if r.installed_digest == digest(&before) {
                after = r.original;
            }
        }
    }
    let new: Vec<_> = if op == "install" {
        (if o.agent == Agent::ClaudeCode {
            CLAUDE
        } else {
            CODEX
        })
        .iter()
        .map(|e| json!({"event":e,"handler":handler(o.agent,e,&o.exe,&o.runtime)}))
        .collect()
    } else {
        vec![]
    };
    let warnings = if o.agent == Agent::Codex {
        vec![
            "请在 Codex 中审阅并信任新 hook 定义；升级后也需要重新审阅。安装器不会修改信任记录。"
                .into(),
        ]
    } else {
        vec!["请在受信任项目中新开 Claude Code 会话进行验证。".into()]
    };
    Ok((
        InstallPreview {
            agent: o.agent,
            operation: op.into(),
            target: path.to_string_lossy().into(),
            digest: digest(&before),
            changes: serde_json::to_string_pretty(
                &json!({"operation":op,"agent":o.agent,"runtime":o.runtime,"hooks":new}),
            )
            .unwrap(),
            warnings,
            changed: before != after,
        },
        before,
        after,
    ))
}
pub fn preview(o: &Options, op: &str) -> Result<InstallPreview, String> {
    Ok(build(o, op)?.0)
}
pub fn apply(o: &Options, op: &str, expected: &str) -> Result<InstallPreview, String> {
    let (p, before, after) = build(o, op)?;
    if p.digest != expected {
        return Err("配置已被其他程序修改，请重新预览".into());
    }
    if !p.changed {
        return Ok(p);
    }
    let path = PathBuf::from(&p.target);
    let rp = receipt(o, &path);
    let old_receipt = fs::read(&rp).ok();
    if op == "install" {
        let previous = fs::read(&rp)
            .ok()
            .and_then(|b| serde_json::from_slice::<Receipt>(&b).ok());
        let original = match previous {
            Some(r) if r.installed_digest == digest(&before) => r.original,
            Some(_) if !path.exists() => Vec::new(),
            Some(_) => {
                let (_, v) = load(&path)?;
                encode(&path, &before, &transform(v, o, false)?)?
            }
            None => before.clone(),
        };
        atomic_write(
            &rp,
            &serde_json::to_vec(&Receipt {
                original,
                installed_digest: digest(&after),
                target: path.clone(),
                root: o.root.clone(),
                agent: Some(o.agent),
                runtime: o.runtime.clone(),
            })
            .unwrap(),
        )?;
    }
    let write_result = (|| -> Result<(), String> {
        if path.exists() {
            atomic_write(
                &path.with_extension(format!(
                    "{}-{}.bak",
                    path.extension().unwrap_or_default().to_string_lossy(),
                    uuid::Uuid::new_v4()
                )),
                &before,
            )?;
        }
        if op == "uninstall" && after.is_empty() {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| e.to_string())?;
            }
        } else {
            atomic_write(&path, &after)?;
        }
        Ok(())
    })();
    if let Err(error) = write_result {
        if op == "install" {
            let restored = if let Some(bytes) = old_receipt {
                atomic_write(&rp, &bytes)
            } else {
                fs::remove_file(&rp).map_err(|e| e.to_string())
            };
            if let Err(rollback) = restored {
                return Err(format!("{error}；回执回滚失败：{rollback}"));
            }
        }
        return Err(error);
    }
    if op == "uninstall" {
        let _ = fs::remove_file(rp);
    }
    Ok(p)
}
pub fn status(o: &Options) -> Result<HookStatus, String> {
    let path = target(o)?;
    let (_, v) = load(&path)?;
    let installed = v["hooks"].as_object().is_some_and(|m| {
        m.values().any(|g| {
            g.as_array().is_some_and(|a| {
                a.iter().any(|g| {
                    g["hooks"]
                        .as_array()
                        .is_some_and(|hs| hs.iter().any(|h| owned(h, o)))
                })
            })
        })
    });
    Ok(HookStatus {
        agent: o.agent,
        target: path.to_string_lossy().into(),
        installed,
        needs_trust: installed && o.agent == Agent::Codex,
        runtime: o.runtime.clone(),
    })
}

/// Called by the uninstaller. Only receipts for this runtime are eligible; trust files are never read or written.
pub fn cleanup(state_dir: &Path, runtime: &str, exe: &Path) -> Result<usize, String> {
    cleanup_filtered(state_dir, runtime, exe, None)
}
pub fn cleanup_filtered(
    state_dir: &Path,
    runtime: &str,
    exe: &Path,
    only_agent: Option<Agent>,
) -> Result<usize, String> {
    let entries = match fs::read_dir(state_dir.join("installs")) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(e.to_string()),
    };
    let mut count = 0;
    let mut failures = Vec::new();
    for entry in entries.flatten() {
        let Ok(bytes) = fs::read(entry.path()) else {
            continue;
        };
        let Ok(r) = serde_json::from_slice::<Receipt>(&bytes) else {
            continue;
        };
        let Some(agent) = r.agent else {
            continue;
        };
        if r.runtime != runtime
            || !r.root.is_absolute()
            || only_agent.is_some_and(|wanted| wanted != agent)
        {
            continue;
        }
        let o = Options {
            agent,
            root: r.root,
            exe: exe.into(),
            runtime: runtime.into(),
            state_dir: state_dir.into(),
        };
        // A moved config needs manual review; never redirect a receipt to a different file.
        if target(&o).ok().as_ref() != Some(&r.target) || receipt(&o, &r.target) != entry.path() {
            failures.push("配置入口已变化".to_string());
            continue;
        }
        match preview(&o, "uninstall").and_then(|p| apply(&o, "uninstall", &p.digest)) {
            Ok(_) => {
                let _ = fs::remove_file(entry.path());
                count += 1;
            }
            Err(e) => failures.push(e),
        }
    }
    if failures.is_empty() {
        Ok(count)
    } else {
        Err(failures.join("；"))
    }
}
