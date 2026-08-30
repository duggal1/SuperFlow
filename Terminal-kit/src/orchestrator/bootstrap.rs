use super::*;

impl Orchestrator {
    pub fn open(state_or_db_path: &Path) -> Result<Self> {
        let state_dir = if state_or_db_path.is_dir() {
            state_or_db_path.to_owned()
        } else if let Some(parent) = state_or_db_path.parent() {
            parent.to_owned()
        } else {
            state_or_db_path.to_owned()
        };

        ensure_state_tree(&state_dir)?;
        cleanup_legacy_hidden_state_dir(&state_dir)?;

        let store = Store::open(&state_dir)?;
        Ok(Self {
            store,
            prompts: PromptLibrary::load(),
        })
    }

    pub fn bootstrap(config: &LaunchConfig) -> Result<Self> {
        ensure_state_tree(&config.state_dir)?;
        cleanup_legacy_hidden_state_dir(&config.state_dir)?;

        let store = Store::open(&config.state_dir)?;
        Ok(Self {
            store,
            prompts: PromptLibrary::load(),
        })
    }
}

pub(super) fn ensure_agents_bootstrap(
    repo: &Path,
    prompts: &PromptLibrary,
) -> Result<AgentsBootstrap> {
    let path = repo.join("AGENTS.md");
    let existed = path.exists();
    let _content = if existed {
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?
    } else {
        let rendered = generate_agents_md(repo, prompts)?;
        fs::write(&path, &rendered)
            .with_context(|| format!("failed to write {}", path.display()))?;
        rendered
    };
    Ok(AgentsBootstrap { path, existed })
}

pub(super) fn render_supervisor_prompt_with_agents(
    base_prompt: String,
    agents_bootstrap: &AgentsBootstrap,
    requested_workers: usize,
) -> String {
    prompt_contracts::render_supervisor_bootstrap(
        base_prompt,
        &agents_bootstrap.path,
        agents_bootstrap.existed,
        requested_workers,
    )
}

pub(super) fn render_worker_prompt_with_agents(
    base_prompt: String,
    agents_bootstrap: &AgentsBootstrap,
    state_dir: &Path,
    packet: &WorkerPacket,
    git_remote: Option<&str>,
    memory_block: Option<&str>,
) -> String {
    prompt_contracts::render_worker_bootstrap(
        base_prompt,
        &agents_bootstrap.path,
        state_dir,
        packet,
        git_remote,
        memory_block,
    )
}

pub(super) fn generate_agents_md(repo: &Path, prompts: &PromptLibrary) -> Result<String> {
    let repo_name = repo
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("repository");
    let cargo_toml = repo.join("Cargo.toml");
    let package_json = repo.join("package.json");
    let go_mod = repo.join("go.mod");
    let pyproject = repo.join("pyproject.toml");

    let (stack_summary, build_cmd, run_cmd, test_cmd, entry_point) = if cargo_toml.exists() {
        (
            "Rust CLI".to_owned(),
            "cargo build".to_owned(),
            "cargo run -- --help".to_owned(),
            "cargo test".to_owned(),
            detect_entry_point(repo, &["src/main.rs", "src/lib.rs", "main.rs"]),
        )
    } else if package_json.exists() {
        (
            "Node.js project".to_owned(),
            "npm run build".to_owned(),
            "npm run dev".to_owned(),
            "npm test".to_owned(),
            detect_entry_point(
                repo,
                &["src/index.ts", "src/index.js", "index.ts", "index.js"],
            ),
        )
    } else if go_mod.exists() {
        (
            "Go project".to_owned(),
            "go build ./...".to_owned(),
            "go run .".to_owned(),
            "go test ./...".to_owned(),
            detect_entry_point(repo, &["main.go", "cmd/server/main.go", "cmd/main.go"]),
        )
    } else if pyproject.exists() {
        (
            "Python project".to_owned(),
            "python -m build".to_owned(),
            "python -m <entrypoint>".to_owned(),
            "pytest".to_owned(),
            detect_entry_point(repo, &["src/__init__.py", "main.py", "app.py"]),
        )
    } else {
        (
            "Repository under active development".to_owned(),
            "inspect local build tooling".to_owned(),
            "inspect repo entrypoint".to_owned(),
            "inspect local test tooling".to_owned(),
            "unknown".to_owned(),
        )
    };

    let top_level = render_repo_tree(repo)?;
    let key_modules = render_module_table(repo)?;
    let critical_files = render_critical_files(repo)?;

    Ok(format!(
        "# AGENTS.md\n\nRead this file before starting any task. It is the local operating guide for this repository.\n\n## Product Overview\n\n- Product: `{repo_name}`\n- Type: {stack_summary}\n- Status: Active development\n- Entry point: `{entry_point}`\n- Source template: `agents.md-instructions.md`\n\n## Tech Stack\n\n- Primary stack: {stack_summary}\n- Build: `{build_cmd}`\n- Run: `{run_cmd}`\n- Test: `{test_cmd}`\n\n## Architecture\n\n- Work from the actual repository state, not assumptions.\n- Trace entrypoints, orchestration boundaries, persistence, and runtime control paths before broad edits.\n- Keep module ownership tight and avoid hidden cross-cutting changes.\n\n## Repository File Tree\n\n```text\n{top_level}\n```\n\n## Critical File Index\n\n{critical_files}\n\n## Module Boundaries\n\n{key_modules}\n\n## Known Issues & Active Debt\n\n- This file may be auto-seeded and should be refreshed when architecture, workflow, or runtime behavior changes materially.\n- Prefer concrete repo evidence over stale summaries.\n\n## Agent Notes\n\n- Read this file once at initialization before real work.\n- Do not expand scope without explicit justification.\n- If ownership is unclear, escalate instead of guessing.\n\n## Operating Protocol\n\n- Reproduce or inspect the current state before broad edits.\n- Keep changes inside the owned scope.\n- State exact files, exact checks, and exact remaining risk.\n- Refresh this file when repository reality changes materially.\n\n## Testing & Validation\n\n- Run the smallest relevant checks that prove the claim.\n- Do not mark work complete without observed results.\n- Call out what remains unverified.\n\n## Guardrails\n\n- No decorative churn.\n- No hidden rewrites.\n- No silent scope expansion.\n- No fake completion claims.\n\n## Instruction Source\n\nThe authoritative formatting source used to seed this file is embedded in Sapphire from `agents.md-instructions.md`.\nUse that source when this file must be materially refreshed.\nCurrent embedded template size: {instruction_size} bytes.\n\n---\n\nAuto-seeded by Sapphire from the AGENTS instruction source. Refresh this file when the repository changes materially.\n",
        instruction_size = prompts.agents_instruction_source().len(),
    ))
}

pub(super) fn detect_entry_point(repo: &Path, candidates: &[&str]) -> String {
    candidates
        .iter()
        .find(|candidate| repo.join(candidate).exists())
        .map(|candidate| candidate.to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

pub(super) fn render_repo_tree(repo: &Path) -> Result<String> {
    let mut entries = fs::read_dir(repo)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') && name != ".github" {
                None
            } else {
                Some((name, entry.path().is_dir()))
            }
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut lines = vec!["/".to_owned()];
    for (name, is_dir) in entries.into_iter().take(24) {
        lines.push(format!("├── {}{}", name, if is_dir { "/" } else { "" }));
    }
    Ok(lines.join("\n"))
}

pub(super) fn render_module_table(repo: &Path) -> Result<String> {
    let mut modules = fs::read_dir(repo)?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !entry.path().is_dir() || name.starts_with('.') {
                None
            } else {
                Some(name)
            }
        })
        .collect::<Vec<_>>();
    modules.sort();
    if modules.is_empty() {
        return Ok("- No top-level module directories detected automatically.".to_owned());
    }
    Ok(modules
        .into_iter()
        .take(8)
        .map(|module| format!("- `{module}`: inspect this directory before touching related code."))
        .collect::<Vec<_>>()
        .join("\n"))
}

pub(super) fn render_critical_files(repo: &Path) -> Result<String> {
    let mut lines = Vec::new();
    for candidate in [
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pyproject.toml",
        "src/main.rs",
        "src/lib.rs",
        "README.md",
    ] {
        if repo.join(candidate).exists() {
            lines.push(format!("- `{candidate}`"));
        }
    }
    if lines.is_empty() {
        lines.push("- No standard critical files detected automatically.".to_owned());
    }
    Ok(lines.join("\n"))
}

pub(super) fn write_prompt_file(state_dir: &Path, session_name: &str, prompt: &str) -> Result<()> {
    let path = launch_prompt::prompt_file_path(state_dir, session_name);
    write_string_to_file(&path, prompt)
}

pub(super) fn ensure_worker_status_path(primary_path: &Path) -> Result<()> {
    if let Some(parent) = primary_path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

pub(super) fn ensure_state_tree(state_dir: &Path) -> Result<()> {
    fs::create_dir_all(state_dir)
        .with_context(|| format!("failed to create {}", state_dir.display()))?;
    fs::create_dir_all(state_dir.join("prompts"))?;
    fs::create_dir_all(state_dir.join("control"))?;
    fs::create_dir_all(state_dir.join("transcripts"))?;
    fs::create_dir_all(state_dir.join("workers"))?;
    fs::create_dir_all(state_dir.join("forge-home/.local/share"))?;
    fs::create_dir_all(state_dir.join("forge-home/.config"))?;
    fs::create_dir_all(state_dir.join("supervisor-runtime"))?;
    Ok(())
}

fn cleanup_legacy_hidden_state_dir(state_dir: &Path) -> Result<()> {
    let Some(hidden_dir) = legacy_hidden_state_dir(state_dir) else {
        return Ok(());
    };
    if hidden_dir.exists() {
        fs::remove_dir_all(&hidden_dir).with_context(|| {
            format!("failed to remove legacy state dir {}", hidden_dir.display())
        })?;
    }
    Ok(())
}

fn legacy_hidden_state_dir(state_dir: &Path) -> Option<PathBuf> {
    let parent = state_dir.parent().unwrap_or_else(|| Path::new("."));
    let hidden = match state_dir.file_name().and_then(|name| name.to_str()) {
        Some(".sp") => parent.join(".hide.sp"),
        Some(name) => parent.join(format!(".hide.{name}")),
        None => return None,
    };
    Some(hidden)
}
