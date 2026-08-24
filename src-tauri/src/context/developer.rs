use std::path::Path;
use std::process::Command;

const MAX_STATUS_ENTRIES: usize = 24;
const MAX_STATUS_LINE_CHARS: usize = 240;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeveloperContext {
    pub project_name: String,
    pub branch: Option<String>,
    pub changed_files: Vec<String>,
    pub instruction_files: Vec<String>,
}

impl DeveloperContext {
    pub fn capture(root: &Path) -> Self {
        let project_name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string();
        let (branch, changed_files) = git_status(root);
        let instruction_files = ["AGENTS.md", "CLAUDE.md", ".claude/CLAUDE.md"]
            .into_iter()
            .filter(|relative| root.join(relative).is_file())
            .map(str::to_string)
            .collect();

        Self {
            project_name,
            branch,
            changed_files,
            instruction_files,
        }
    }

    pub fn render(&self) -> String {
        let mut lines = vec![format!("Project: {}", self.project_name)];
        if let Some(branch) = &self.branch {
            lines.push(format!("Git branch: {branch}"));
        }
        if !self.instruction_files.is_empty() {
            lines.push(format!(
                "Repository instructions: {}",
                self.instruction_files.join(", ")
            ));
        }
        if !self.changed_files.is_empty() {
            lines.push("Tracked working-tree changes:".to_string());
            lines.extend(self.changed_files.iter().map(|entry| format!("- {entry}")));
        }
        lines.join("\n")
    }
}

fn git_status(root: &Path) -> (Option<String>, Vec<String>) {
    let Ok(output) = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "status",
            "--porcelain=v1",
            "--branch",
            "--untracked-files=no",
        ])
        .output()
    else {
        return (None, Vec::new());
    };
    if !output.status.success() {
        return (None, Vec::new());
    }

    parse_git_status(&String::from_utf8_lossy(&output.stdout))
}

fn parse_git_status(status: &str) -> (Option<String>, Vec<String>) {
    let mut lines = status.lines();
    let branch = lines.next().and_then(|header| {
        header
            .strip_prefix("## ")
            .and_then(|value| value.split("...").next())
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "HEAD (no branch)")
            .map(str::to_string)
    });
    let changed_files = lines
        .take(MAX_STATUS_ENTRIES)
        .map(sanitize_status_line)
        .filter(|line| !line.is_empty())
        .collect();
    (branch, changed_files)
}

fn sanitize_status_line(line: &str) -> String {
    line.chars()
        .filter(|character| !character.is_control())
        .take(MAX_STATUS_LINE_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_branch_and_bounded_tracked_changes() {
        let status = "## main...origin/main\n M src/actions.rs\nR  old.rs -> new.rs\n";
        let (branch, changes) = parse_git_status(status);
        assert_eq!(branch.as_deref(), Some("main"));
        assert_eq!(changes, ["M src/actions.rs", "R  old.rs -> new.rs"]);
    }

    #[test]
    fn rendered_context_contains_metadata_not_file_contents() {
        let context = DeveloperContext {
            project_name: "SuperFlow".into(),
            branch: Some("main".into()),
            changed_files: vec!["M src/actions.rs".into()],
            instruction_files: vec!["AGENTS.md".into()],
        };
        let rendered = context.render();
        assert!(rendered.contains("Project: SuperFlow"));
        assert!(rendered.contains("Git branch: main"));
        assert!(rendered.contains("Repository instructions: AGENTS.md"));
        assert!(rendered.contains("M src/actions.rs"));
    }
}
