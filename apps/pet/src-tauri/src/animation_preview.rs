use code_musume_contracts::{AnimationPreview, DisplayState};

pub struct PreviewController {
    value: AnimationPreview,
}

impl PreviewController {
    pub fn new(enabled: bool) -> Self {
        Self {
            value: AnimationPreview {
                enabled,
                state: None,
                revision: 0,
            },
        }
    }

    pub fn snapshot(&self) -> AnimationPreview {
        self.value.clone()
    }

    pub fn active(&self) -> bool {
        self.value.enabled && self.value.state.is_some()
    }

    pub fn set_enabled(&mut self, enabled: bool) -> bool {
        if self.value.enabled == enabled {
            return false;
        }
        self.value.enabled = enabled;
        self.value.state = None;
        self.value.revision += 1;
        true
    }

    pub fn select(&mut self, state: Option<DisplayState>) -> Result<bool, String> {
        if !self.value.enabled && state.is_some() {
            return Err("请先开启手动状态预览".into());
        }
        if self.value.state == state {
            return Ok(false);
        }
        self.value.state = state;
        self.value.revision += 1;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use code_musume_contracts::Config;

    #[test]
    fn disabled_preview_rejects_selection_and_return_keeps_feature_enabled() {
        let mut preview = PreviewController::new(false);
        assert!(preview.select(Some(DisplayState::Working)).is_err());
        assert_eq!(preview.snapshot().revision, 0);
        assert!(preview.set_enabled(true));
        assert_eq!(preview.snapshot().state, None);
        assert!(preview.select(Some(DisplayState::Working)).unwrap());
        assert!(preview.active());
        let revision = preview.snapshot().revision;
        assert!(!preview.select(Some(DisplayState::Working)).unwrap());
        assert_eq!(preview.snapshot().revision, revision);
        assert!(preview.select(None).unwrap());
        assert!(preview.snapshot().enabled);
        assert!(!preview.active());
    }

    #[test]
    fn disabling_clears_selection_and_restart_restores_only_enabled_preference() {
        let mut preview = PreviewController::new(true);
        preview.select(Some(DisplayState::Sleeping)).unwrap();
        let restarted = PreviewController::new(preview.snapshot().enabled);
        assert!(restarted.snapshot().enabled);
        assert_eq!(restarted.snapshot().state, None);
        let revision = preview.snapshot().revision;
        assert!(preview.set_enabled(false));
        assert_eq!(preview.snapshot().state, None);
        assert!(preview.snapshot().revision > revision);
        assert!(preview.set_enabled(true));
        assert_eq!(preview.snapshot().state, None);
    }

    #[test]
    fn old_configuration_defaults_to_disabled_and_never_contains_selected_state() {
        let mut config: Config = serde_json::from_str(r#"{"schemaVersion":1}"#).unwrap();
        assert!(!config.manual_animation_enabled);
        config.manual_animation_enabled = true;
        let json = serde_json::to_value(config).unwrap();
        assert_eq!(json["manualAnimationEnabled"], true);
        assert!(json.get("state").is_none());
        assert!(serde_json::from_value::<DisplayState>(serde_json::json!("invalid")).is_err());
    }
}
