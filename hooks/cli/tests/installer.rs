use code_musume_contracts::Agent;
use code_musume_hook::installer::{self, Options};
use std::{fs, path::PathBuf};
struct Sandbox {
    path: PathBuf,
}
impl Sandbox {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cm-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
    fn options(&self, agent: Agent) -> Options {
        let exe = self
            .path
            .join("中文 空格 & ' % !")
            .join("code-musume-hook.exe");
        fs::create_dir_all(exe.parent().unwrap()).unwrap();
        fs::write(&exe, []).unwrap();
        Options {
            agent,
            root: self.path.clone(),
            exe,
            runtime: "dev".into(),
            state_dir: self.path.join("state"),
        }
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        if let (Ok(p), Ok(t)) = (
            self.path.canonicalize(),
            std::env::temp_dir().canonicalize(),
        ) {
            if p.starts_with(t)
                && p.file_name()
                    .is_some_and(|s| s.to_string_lossy().starts_with("cm-test-"))
            {
                let _ = fs::remove_dir_all(p);
            }
        }
    }
}
fn run(o: &Options, op: &str) {
    let p = installer::preview(o, op).unwrap();
    installer::apply(o, op, &p.digest).unwrap();
}
#[test]
fn three_installs_are_idempotent_and_uninstall_restores_bytes() {
    for agent in [Agent::ClaudeCode, Agent::Codex] {
        let t = Sandbox::new();
        let o = t.options(agent);
        let path = t.path.join(if agent == Agent::ClaudeCode {
            ".claude/settings.json"
        } else {
            ".codex/hooks.json"
        });
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let original=b"{\n  \"custom\": 42, \"hooks\":{\"Stop\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"other\"}]}]}\n}";
        fs::write(&path, original).unwrap();
        run(&o, "install");
        let once = fs::read(&path).unwrap();
        run(&o, "install");
        run(&o, "install");
        assert_eq!(fs::read(&path).unwrap(), once);
        run(&o, "uninstall");
        assert_eq!(fs::read(&path).unwrap(), original);
    }
}
#[test]
fn edits_after_install_survive_uninstall_and_stale_previews_are_rejected() {
    let t = Sandbox::new();
    let o = t.options(Agent::ClaudeCode);
    run(&o, "install");
    let path = t.path.join(".claude/settings.json");
    let p = installer::preview(&o, "uninstall").unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    v["newUserSetting"] = true.into();
    fs::write(&path, serde_json::to_vec(&v).unwrap()).unwrap();
    assert!(installer::apply(&o, "uninstall", &p.digest).is_err());
    run(&o, "uninstall");
    let v: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(v["newUserSetting"], true);
    assert!(v.get("hooks").is_none());
}
#[test]
fn malformed_files_are_not_written() {
    let t = Sandbox::new();
    let o = t.options(Agent::Codex);
    let p = t.path.join(".codex/hooks.json");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, "{").unwrap();
    assert!(installer::preview(&o, "install").is_err());
    assert_eq!(fs::read_to_string(p).unwrap(), "{");
}
#[test]
fn existing_inline_toml_is_used_and_outer_format_is_preserved() {
    let t = Sandbox::new();
    let o = t.options(Agent::Codex);
    let p = t.path.join(".codex/config.toml");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    let before="# keep my comment\nmodel = 'example'\n\n[[hooks.Stop]]\n[[hooks.Stop.hooks]]\ntype = 'command'\ncommand = 'other'\n";
    fs::write(&p, before).unwrap();
    let preview = installer::preview(&o, "install").unwrap();
    assert!(preview.target.ends_with("config.toml"));
    run(&o, "install");
    let text = fs::read_to_string(&p).unwrap();
    assert!(text.starts_with("# keep my comment\nmodel = 'example'"));
    assert!(text.contains("other"));
    assert!(!t.path.join(".codex/hooks.json").exists());
    run(&o, "uninstall");
    assert_eq!(fs::read_to_string(p).unwrap(), before);
}
#[test]
fn sources_and_runtime_modes_uninstall_independently() {
    let t = Sandbox::new();
    let a = t.options(Agent::ClaudeCode);
    let mut b = t.options(Agent::Codex);
    run(&a, "install");
    run(&b, "install");
    b.runtime = "release".into();
    run(&b, "install");
    b.runtime = "dev".into();
    run(&b, "uninstall");
    assert!(installer::status(&a).unwrap().installed);
    b.runtime = "release".into();
    assert!(installer::status(&b).unwrap().installed);
}

#[test]
fn upgrade_after_external_edit_does_not_restore_old_hooks() {
    let t = Sandbox::new();
    let mut o = t.options(Agent::Codex);
    run(&o, "install");
    let p = t.path.join(".codex/hooks.json");
    let mut v: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    v["userSetting"] = true.into();
    v["hooks"]["CustomEmpty"] = serde_json::json!([]);
    fs::write(&p, serde_json::to_vec(&v).unwrap()).unwrap();
    let next = t.path.join("upgrade/code-musume-hook.exe");
    fs::create_dir_all(next.parent().unwrap()).unwrap();
    fs::write(&next, []).unwrap();
    o.exe = next;
    run(&o, "install");
    run(&o, "uninstall");
    let v: serde_json::Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
    assert_eq!(v["userSetting"], true);
    assert_eq!(v["hooks"], serde_json::json!({"CustomEmpty":[]}));
    assert!(!installer::status(&o).unwrap().installed);
}
#[test]
fn uninstall_cleanup_is_scoped_to_receipts_and_runtime() {
    let t = Sandbox::new();
    let a = t.options(Agent::ClaudeCode);
    let mut b = t.options(Agent::Codex);
    b.runtime = "release".into();
    run(&a, "install");
    run(&b, "install");
    assert_eq!(
        installer::cleanup(&b.state_dir, "release", &b.exe).unwrap(),
        1
    );
    assert!(installer::status(&a).unwrap().installed);
    assert!(!installer::status(&b).unwrap().installed);
}

#[test]
fn reinstall_after_manual_deletion_preserves_absence_on_uninstall() {
    let t = Sandbox::new();
    let o = t.options(Agent::ClaudeCode);
    run(&o, "install");
    let path = t.path.join(".claude/settings.json");
    fs::remove_file(&path).unwrap();
    run(&o, "install");
    run(&o, "uninstall");
    assert!(!path.exists());
}

#[test]
fn cleanup_filter_does_not_remove_other_agent() {
    let t = Sandbox::new();
    let a = t.options(Agent::ClaudeCode);
    let b = t.options(Agent::Codex);
    run(&a, "install");
    run(&b, "install");
    assert_eq!(
        installer::cleanup_filtered(&a.state_dir, "dev", &a.exe, Some(Agent::Codex)).unwrap(),
        1
    );
    assert!(installer::status(&a).unwrap().installed);
    assert!(!installer::status(&b).unwrap().installed);
}
