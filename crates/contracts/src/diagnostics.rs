//! Content-free diagnostics. Never serialize parser messages or input samples.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDiagnostics {
    pub bytes: usize,
    pub utf8_valid: bool,
    pub utf8_bom: bool,
    pub json_valid: bool,
    pub error_kind: Option<&'static str>,
    pub error_line: Option<usize>,
    pub error_column: Option<usize>,
}
impl InputDiagnostics {
    pub fn inspect(bytes: &[u8]) -> Self {
        let mut result = Self {
            bytes: bytes.len(),
            utf8_valid: std::str::from_utf8(bytes).is_ok(),
            utf8_bom: bytes.starts_with(&[0xef, 0xbb, 0xbf]),
            json_valid: false,
            error_kind: None,
            error_line: None,
            error_column: None,
        };
        match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(_) if result.utf8_valid => result.json_valid = true,
            Ok(_) => result.error_kind = Some("utf8"),
            Err(error) => {
                result.error_kind = Some(if result.utf8_bom {
                    "utf8-bom"
                } else if !result.utf8_valid {
                    "utf8"
                } else {
                    match error.classify() {
                        serde_json::error::Category::Eof => "json-eof",
                        serde_json::error::Category::Syntax => "json-syntax",
                        serde_json::error::Category::Data => "json-data",
                        serde_json::error::Category::Io => "json-io",
                    }
                });
                result.error_line = Some(error.line());
                result.error_column = Some(error.column());
            }
        }
        result
    }
}

pub fn safe_event(event: &str) -> &str {
    match event {
        "SessionStart" | "SessionEnd" | "UserPromptSubmit" | "PreToolUse" | "PostToolUse"
        | "PostToolUseFailure" | "PermissionRequest" | "PermissionDenied" | "Stop"
        | "StopFailure" | "Interrupt" | "SubagentStart" | "SubagentStop" | "PreCompact"
        | "PostCompact" | "Notification" | "transport" => event,
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_encoding_syntax_and_truncation_without_retaining_content() {
        let secret = "private-prompt-and-token-C:/Users/private";
        let valid = format!(r#"{{"prompt":"{secret}"}}"#);
        let good = InputDiagnostics::inspect(valid.as_bytes());
        assert!(good.json_valid);
        let mut bom = vec![0xef, 0xbb, 0xbf];
        bom.extend(valid.as_bytes());
        let report = InputDiagnostics::inspect(&bom);
        assert_eq!(report.error_kind, Some("utf8-bom"));
        assert!(report.utf8_bom);
        assert_eq!(InputDiagnostics::inspect(&[0xff]).error_kind, Some("utf8"));
        assert_eq!(
            InputDiagnostics::inspect(b"{\"prompt\":").error_kind,
            Some("json-eof")
        );
        let saved = serde_json::to_string(&report).unwrap();
        assert!(!saved.contains(secret) && !saved.contains("prompt") && !saved.contains("token"));
        assert_eq!(safe_event(secret), "other");
    }
}
