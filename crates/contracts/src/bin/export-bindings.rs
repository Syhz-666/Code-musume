use code_musume_contracts::*;
use std::{fs, path::PathBuf};
use ts_rs::TS;
fn main() {
    let target = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/protocol/src/generated.ts")
        });
    let mut output = format!("// Generated from Rust contracts. Run pnpm contracts:generate; do not edit.\nexport const PROTOCOL_VERSION = {PROTOCOL_VERSION};\n");
    let cfg = ts_rs::Config::default();
    macro_rules! export { ($($t:ty),*) => { $(output.push_str(&format!("export {}\n", <$t>::decl(&cfg)));)* }; }
    export!(
        Agent,
        EnvelopeSrc,
        ParseError,
        EnvelopeV2,
        DisplayState,
        Certainty,
        AgentCapabilities,
        SessionSummary,
        Projection,
        Position,
        Config,
        AnimationPreview,
        Diagnostic,
        RenderReceipt,
        Clip,
        CharacterForm,
        CharacterManifest,
        InstallPreview,
        HookStatus
    );
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(target, output).unwrap();
}
