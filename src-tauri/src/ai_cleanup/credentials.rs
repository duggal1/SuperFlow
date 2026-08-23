const SERVICE: &str = "com.superflow.app.ai-cleanup";
const ACCOUNT: &str = "gemini-api-key";
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

#[cfg(target_os = "macos")]
fn stored() -> Result<Option<String>, String> {
    use security_framework::passwords::get_generic_password;

    match get_generic_password(SERVICE, ACCOUNT) {
        Ok(bytes) => String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| "Gemini API key in macOS Keychain is invalid".to_string()),
        Err(error) if error.code() == -25_300 => Ok(None),
        Err(_) => Err("Gemini API key could not be read from macOS Keychain".to_string()),
    }
}

#[cfg(not(target_os = "macos"))]
fn stored() -> Result<Option<String>, String> {
    Ok(None)
}

pub fn load() -> Result<String, String> {
    resolve(stored()?, std::env::var("GEMINI_API_KEY").ok())
        .ok_or_else(|| MISSING_API_KEY_ERROR.to_string())
}

pub fn is_configured() -> Result<bool, String> {
    Ok(resolve(stored()?, std::env::var("GEMINI_API_KEY").ok()).is_some())
}

#[cfg(target_os = "macos")]
pub fn save(api_key: &str) -> Result<(), String> {
    use security_framework::passwords::{delete_generic_password, set_generic_password};

    let api_key = api_key.trim();
    if api_key.chars().count() > MAX_API_KEY_CHARS {
        return Err("Gemini API key is too long".to_string());
    }
    if api_key.is_empty() {
        return match delete_generic_password(SERVICE, ACCOUNT) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == -25_300 => Ok(()),
            Err(_) => Err("Gemini API key could not be removed from macOS Keychain".to_string()),
        };
    }
    set_generic_password(SERVICE, ACCOUNT, api_key.as_bytes())
        .map_err(|_| "Gemini API key could not be saved to macOS Keychain".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn save(_api_key: &str) -> Result<(), String> {
    Err("Gemini API key storage is only available on macOS".to_string())
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
    fn missing_key_error_matches_the_user_facing_dialog() {
        assert_eq!(
            MISSING_API_KEY_ERROR,
            "Error: please configure your API Key first."
        );
    }
}
