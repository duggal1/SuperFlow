use crate::settings::AppSettings;

pub const SETTINGS_KEY: &str = "gemini";
const MAX_API_KEY_CHARS: usize = 4_096;
pub(super) const MISSING_API_KEY_ERROR: &str = "Error: please configure your API Key first.";

fn normalized(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_string())
    })
}

fn resolve(stored: Option<String>, environment: Option<String>) -> Option<String> {
    normalized(stored).or_else(|| normalized(environment))
}

pub fn load(settings: &AppSettings) -> Result<String, String> {
    resolve(
        settings.post_process_api_keys.get(SETTINGS_KEY).cloned(),
        std::env::var("GEMINI_API_KEY").ok(),
    )
    .ok_or_else(|| MISSING_API_KEY_ERROR.to_string())
}

pub fn is_configured(settings: &AppSettings) -> bool {
    resolve(
        settings.post_process_api_keys.get(SETTINGS_KEY).cloned(),
        std::env::var("GEMINI_API_KEY").ok(),
    )
    .is_some()
}

pub fn save(settings: &mut AppSettings, api_key: &str) -> Result<(), String> {
    let api_key = api_key.trim();
    if api_key.chars().count() > MAX_API_KEY_CHARS {
        return Err("Gemini API key is too long".to_string());
    }
    settings
        .post_process_api_keys
        .insert(SETTINGS_KEY.to_string(), api_key.to_string());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_key_takes_precedence_over_environment() {
        assert_eq!(
            resolve(Some(" stored ".to_string()), Some("env".to_string())),
            Some("stored".to_string())
        );
    }

    #[test]
    fn blank_sources_are_not_configured() {
        assert_eq!(resolve(Some("  ".to_string()), Some(String::new())), None);
    }

    #[test]
    fn key_is_saved_in_the_existing_settings_secret_map() {
        let mut settings = crate::settings::get_default_settings();
        save(&mut settings, "  gemini-key  ").unwrap();
        assert_eq!(
            settings.post_process_api_keys.get(SETTINGS_KEY),
            Some(&"gemini-key".to_string())
        );
    }

    #[test]
    fn missing_key_error_matches_the_user_facing_dialog() {
        assert_eq!(
            MISSING_API_KEY_ERROR,
            "Error: please configure your API Key first."
        );
    }
}
