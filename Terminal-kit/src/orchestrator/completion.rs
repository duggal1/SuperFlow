use super::*;

pub(super) fn reject_completion_without_artifacts(
    repo_root: &Path,
    packet: &WorkerPacket,
    reported_files: &[String],
) -> Option<Vec<String>> {
    let expectation = infer_completion_expectation(packet);
    if expectation.exact_files.is_empty()
        && !expectation.require_readme
        && !expectation.require_script
    {
        return None;
    }

    let observed = scan_repo_artifacts(repo_root, 4096);
    let observed_lower = observed
        .iter()
        .map(|path| path.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let reported_lower = reported_files
        .iter()
        .map(|path| path.trim().to_ascii_lowercase())
        .collect::<BTreeSet<_>>();

    let mut missing = BTreeSet::new();
    for file in &expectation.exact_files {
        let needle = file.to_ascii_lowercase();
        let present = observed_lower.iter().any(|path| path.ends_with(&needle))
            || reported_lower.iter().any(|path| path.ends_with(&needle));
        if !present {
            missing.insert(file.clone());
        }
    }

    if expectation.require_readme
        && !observed_lower
            .iter()
            .any(|path| path.ends_with("readme.md") || path.ends_with("readme"))
    {
        missing.insert("README.md".to_owned());
    }

    if expectation.require_script
        && !observed_lower.iter().any(|path| {
            path.ends_with(".sh")
                || path.ends_with(".py")
                || path.ends_with(".js")
                || path.ends_with(".ts")
        })
    {
        missing.insert("script artifact (*.sh|*.py|*.js|*.ts)".to_owned());
    }

    if missing.is_empty() {
        None
    } else {
        Some(missing.into_iter().collect())
    }
}

pub(super) fn infer_completion_expectation(packet: &WorkerPacket) -> CompletionExpectation {
    let mut expectation = CompletionExpectation::default();
    let mut exact_files = BTreeSet::new();
    let texts = packet
        .definition_of_done
        .iter()
        .chain(packet.required_evidence.iter())
        .chain(std::iter::once(&packet.explicit_task))
        .chain(std::iter::once(&packet.owned_scope))
        .collect::<Vec<_>>();

    for text in texts {
        let lowered = text.to_ascii_lowercase();
        if lowered.contains("readme") {
            expectation.require_readme = true;
        }
        if lowered.contains("script") {
            expectation.require_script = true;
        }
        for token in text.split_whitespace() {
            let candidate = token
                .trim_matches(|ch: char| {
                    ch.is_ascii_punctuation() && ch != '.' && ch != '/' && ch != '_'
                })
                .trim();
            if let Some(file) = normalize_expected_file(candidate)
                && !matches!(file.as_str(), "AGENTS.md" | "tasks.txt")
                && file != ".sp"
            {
                exact_files.insert(file);
            }
        }
    }

    expectation.exact_files = exact_files.into_iter().collect();
    expectation
}

pub(super) fn normalize_expected_file(token: &str) -> Option<String> {
    let candidate = token.trim_matches(|ch: char| ch == '"' || ch == '\'' || ch == '`');
    if candidate.is_empty()
        || candidate.starts_with('.')
        || candidate.ends_with(':')
        || candidate.contains("://")
        || !candidate.contains('.')
    {
        return None;
    }
    let extension = candidate.rsplit('.').next()?.to_ascii_lowercase();
    let allowed = [
        "md", "sh", "py", "js", "ts", "tsx", "jsx", "rs", "toml", "json", "yaml", "yml", "sql",
        "txt", "html", "css",
    ];
    if !allowed.contains(&extension.as_str()) {
        return None;
    }
    Some(candidate.trim_start_matches("./").to_owned())
}

pub(super) fn scan_repo_artifacts(repo_root: &Path, max_entries: usize) -> Vec<String> {
    let mut results = Vec::new();
    let mut stack = vec![repo_root.to_path_buf()];

    while let Some(path) = stack.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            if results.len() >= max_entries {
                return results;
            }
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if file_type.is_dir() {
                if matches!(
                    name.as_ref(),
                    ".git" | ".sp" | "target" | "node_modules" | "dist" | "build"
                ) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            if let Ok(relative) = path.strip_prefix(repo_root) {
                results.push(relative.to_string_lossy().into_owned());
            }
        }
    }

    results
}
