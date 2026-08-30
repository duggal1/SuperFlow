use std::fs;
use std::path::Path;
use std::time::SystemTime;

use crate::protocol::StatusDirective;

pub struct StatusFileUpdate {
    pub modified_at: SystemTime,
    pub directive: StatusDirective,
    pub bootstrap: bool,
}

pub fn load_status_file_update(
    status_path: &Path,
    previous_modified: Option<SystemTime>,
) -> Option<StatusFileUpdate> {
    if !status_path.exists() {
        return None;
    }

    let modified_at = fs::metadata(status_path).ok()?.modified().ok()?;
    if previous_modified.is_some_and(|previous| modified_at <= previous) {
        return None;
    }

    let content = fs::read_to_string(status_path).ok()?;
    let status_obj: serde_json::Value = serde_json::from_str(&content).ok()?;
    let state = status_obj.get("state")?.as_str()?.to_owned();
    let summary = status_obj
        .get("summary")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned();
    let files = collect_string_array(status_obj.get("files"));
    let commands = collect_string_array(status_obj.get("commands"));
    let risks = collect_string_array(status_obj.get("risks"));
    let overlap = status_obj
        .get("overlap")
        .and_then(|value| value.as_str())
        .map(str::to_owned);
    let bootstrap = status_obj
        .get("bootstrap")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);

    Some(StatusFileUpdate {
        modified_at,
        directive: StatusDirective {
            state,
            summary,
            files,
            commands,
            risks,
            overlap,
        },
        bootstrap,
    })
}

fn collect_string_array(value: Option<&serde_json::Value>) -> Vec<String> {
    value
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}
