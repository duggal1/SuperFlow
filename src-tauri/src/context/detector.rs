//! Frontmost-application detection via NSWorkspace (macOS).

use objc2_app_kit::NSWorkspace;

pub struct FrontmostApp {
    pub name: String,
    pub bundle_id: Option<String>,
    /// Process identifier — needed to build the AX element for tab inspection.
    pub pid: i32,
}

pub fn frontmost_app() -> Option<FrontmostApp> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let name = app
        .localizedName()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "Unknown".to_string());
    let bundle_id = app.bundleIdentifier().map(|b| b.to_string());
    let pid = app.processIdentifier();
    Some(FrontmostApp {
        name,
        bundle_id,
        pid,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn frontmost_app_returns_something_while_tests_run() {
        // Tests run inside a host process on macOS; detection should at worst
        // return None, never panic.
        let _ = super::frontmost_app();
    }
}
