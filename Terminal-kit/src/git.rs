//! Git pre-flight checks, initialization, and commit locking.
//!
//! Real logic — no placeholders:
//! - Checks if repo has git + remote before launch
//! - Interactive prompt for missing git setup with full Sapphire theming
//! - File-based commit lock prevents overlapping commits from multiple agents
//! - Dirty tree tolerance: agents ignore dirty tree unless completely broken

#![allow(dead_code)]

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::internal::ui::theme::{
    ansi,
    unicode::{Banner, Symbol},
};

// ─── Banner Rendered Once ───────────────────────────────────────────────

/// Tracks whether the Sapphire banner has been printed in this process.
static BANNER_RENDERED: AtomicBool = AtomicBool::new(false);

/// Render the Sapphire ASCII banner with theme colors. Only renders once per process.
fn render_banner() {
    if BANNER_RENDERED.swap(true, Ordering::SeqCst) {
        return;
    }
    println!();

    let banner = Banner::Sapphire;
    let term_width = terminal_width().unwrap_or(100).saturating_sub(2);
    let rendered = banner.centered(term_width);

    for (index, line) in rendered.lines().enumerate() {
        let styled = match index {
            0 | 1 => ansi::brand_soft_bold(line),
            2 | 3 => ansi::brand_bold(line),
            4 | 5 => ansi::blue_bold(line),
            _ => ansi::teal_bold(line),
        };
        println!("  {styled}");
    }
    println!("  {}", ansi::rule(&rule_line()));
    println!();
}

/// Best-effort terminal width detection. Falls back to None.
fn terminal_width() -> Option<usize> {
    use std::process::Command;
    let output = Command::new("tput").arg("cols").output().ok()?;
    if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse::<usize>()
            .ok()
    } else {
        None
    }
}

// ─── Git State Detection ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitState {
    /// The `git` binary is not available in the current environment
    Unavailable,
    /// No .git directory — not a git repo
    NotARepo,
    /// Has .git but no remote configured
    NoRemote,
    /// Has .git and at least one remote
    Ready { remote_url: String },
}

/// Check the git state of a repository directory.
/// Returns fast — one `git rev-parse` + one `git remote` call.
pub fn check_git_state(repo: &Path) -> GitState {
    if !git_binary_available() {
        return GitState::Unavailable;
    }

    if !repo.join(".git").exists() {
        return GitState::NotARepo;
    }

    let rev_parse = Command::new("git")
        .arg("rev-parse")
        .arg("--git-dir")
        .current_dir(repo)
        .output();

    if rev_parse.is_err() || !rev_parse.as_ref().is_ok_and(|o| o.status.success()) {
        return GitState::NotARepo;
    }

    let remote_output = Command::new("git")
        .arg("remote")
        .arg("-v")
        .current_dir(repo)
        .output();

    if let Ok(output) = remote_output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains("(fetch)") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    return GitState::Ready {
                        remote_url: parts[1].to_owned(),
                    };
                }
            }
        }
    }

    GitState::NoRemote
}

// ─── Interactive Git Initialization ─────────────────────────────────────

/// Result of the git initialization prompt.
#[derive(Debug, Clone)]
pub enum GitInitResult {
    /// User said yes, provided remote URL — repo is now initialized
    Initialized { remote_url: String },
    /// User said no — continue without git initialization
    Declined,
}

/// Render the git initialization prompt and collect user input.
/// Shows Sapphire banner once, then styled y/n prompt.
pub fn prompt_git_init(repo: &Path, git_state: &GitState) -> Result<GitInitResult> {
    render_banner();
    render_git_context(git_state);

    if matches!(git_state, GitState::Unavailable) {
        println!(
            "  {}  {}",
            ansi::danger_bold(Symbol::Error.as_str()),
            ansi::muted("Continue without Git, or install it and rerun Sapphire.")
        );
        println!();
        return Ok(GitInitResult::Declined);
    }

    println!(
        "  {}  {}",
        ansi::teal_bold(Symbol::Prompt.as_str()),
        ansi::text_bold("Would you like to initialize a remote repository?")
    );
    println!(
        "     {} {}    {} {}",
        ansi::success_bold("[y]"),
        ansi::text("Yes"),
        ansi::blue_bold("[n]"),
        ansi::muted("No"),
    );
    println!();

    loop {
        let choice = read_input("Choice", "(y/n): ")?;
        match choice.to_ascii_lowercase().as_str() {
            "y" | "yes" => break,
            "n" | "no" | "" => {
                println!();
                println!(
                    "  {}  {}",
                    ansi::blue(Symbol::Info.as_str()),
                    ansi::muted("Continuing without a remote.")
                );
                println!();
                return Ok(GitInitResult::Declined);
            }
            _ => {
                println!(
                    "  {}  {}",
                    ansi::danger_bold(Symbol::Error.as_str()),
                    ansi::danger("Enter y or n.")
                );
                println!();
            }
        }
    }

    println!(
        "  {}  {}",
        ansi::blue_bold(Symbol::Field.as_str()),
        ansi::text_bold("Remote URL")
    );
    println!(
        "     {}",
        ansi::muted("git@github.com:user/repo.git or https://github.com/user/repo.git")
    );
    println!();

    loop {
        let url = read_input("origin", "→ ")?;
        if url.is_empty() {
            println!();
            println!(
                "  {}  {}",
                ansi::blue(Symbol::Info.as_str()),
                ansi::muted("No remote URL entered. Continuing without a remote.")
            );
            println!();
            return Ok(GitInitResult::Declined);
        }
        if looks_like_git_url(&url) {
            return init_and_return(repo, git_state, &url);
        }

        println!(
            "  {}  {}",
            ansi::danger_bold(Symbol::Error.as_str()),
            ansi::danger("That does not look like a valid Git remote URL.")
        );
        println!(
            "     {}",
            ansi::muted(
                "Expected: git@github.com:user/repo.git or https://github.com/user/repo.git"
            )
        );
        println!();
    }
}

fn init_and_return(repo: &Path, git_state: &GitState, url: &str) -> Result<GitInitResult> {
    println!();
    println!(
        "  {}  {}",
        ansi::teal_bold(Symbol::Prompt.as_str()),
        ansi::text(match git_state {
            GitState::NotARepo => "Initializing Git repository and configuring origin…",
            GitState::NoRemote => "Configuring origin remote…",
            _ => "Configuring Git…",
        })
    );

    match init_git_repo(repo, url) {
        Ok(()) => {
            println!();
            println!(
                "  {}  {}",
                ansi::success_bold(Symbol::Success.as_str()),
                ansi::success_bold(match git_state {
                    GitState::NotARepo => "Repository initialized and remote configured",
                    GitState::NoRemote => "Remote configured",
                    _ => "Git configured",
                })
            );
            println!("     {}", ansi::muted(&format!("origin → {url}")));
            println!(
                "     {}",
                ansi::muted("Sapphire will continue with Git collaboration enabled.")
            );
            println!();
            println!("  {}", ansi::rule(&rule_line()));
            println!();
            Ok(GitInitResult::Initialized {
                remote_url: url.to_owned(),
            })
        }
        Err(e) => {
            println!();
            println!(
                "  {}  {}",
                ansi::danger_bold(Symbol::Error.as_str()),
                ansi::danger_bold("Git setup failed")
            );
            println!("     {}", ansi::muted(&e.to_string()));
            println!(
                "     {}",
                ansi::muted("Continuing without Git initialization.")
            );
            println!();
            Ok(GitInitResult::Declined)
        }
    }
}

/// Check if a string looks like a valid git remote URL.
fn looks_like_git_url(url: &str) -> bool {
    if url.starts_with("git@") && url.contains(':') {
        return true;
    }
    if url.starts_with("https://") || url.starts_with("http://") {
        return url.contains("github.com")
            || url.contains("gitlab.com")
            || url.contains("bitbucket.org")
            || url.contains(".git");
    }
    url.contains(".git")
        || url.contains("github.com")
        || url.contains("gitlab.com")
        || url.contains("bitbucket.org")
}

// ─── Git Initialization (Real Commands) ─────────────────────────────────

/// Initialize a git repository and add a remote.
pub fn init_git_repo(repo: &Path, remote_url: &str) -> Result<()> {
    run_git_command(
        repo,
        &["-c", "init.defaultBranch=main", "init", "-q"],
        "git init failed",
    )?;

    run_git_command(
        repo,
        &["remote", "add", "origin", remote_url],
        "git remote add failed",
    )?;

    Ok(())
}

pub fn push_current_branch(repo: &Path) -> Result<()> {
    render_banner();
    match check_git_state(repo) {
        GitState::Unavailable => anyhow::bail!("git is not available in the current environment"),
        GitState::NotARepo => anyhow::bail!("{} is not a git repository", repo.display()),
        GitState::NoRemote => anyhow::bail!(
            "{} has no remote configured. Initialize one first, then rerun `sp push`",
            repo.display()
        ),
        GitState::Ready { .. } => {}
    }

    let branch = current_branch(repo)
        .filter(|value| !value.is_empty())
        .context("failed to determine the current git branch")?;
    if matches!(branch.as_str(), "main" | "master") {
        anyhow::bail!(
            "refusing to push protected branch `{branch}` through Sapphire. Create or switch to a feature branch first"
        );
    }

    let _lock = CommitLock::new(repo)
        .acquire_with_timeout(10)
        .context("timed out waiting for the Sapphire commit lock before push")?;

    println!("  {}", ansi::rule(&rule_line()));
    println!(
        "  {}  {}",
        ansi::brand_soft_bold(Symbol::Prompt.as_str()),
        ansi::text_bold("Sapphire Push")
    );
    println!("  {}", ansi::rule(&rule_line()));
    println!();
    println!("  {}", ansi::muted("Operator-owned push path engaged."));
    println!(
        "  {}",
        ansi::muted(&format!("Repository: {}", repo.display()))
    );
    println!("  {}", ansi::muted(&format!("Branch: {branch}")));
    println!();

    if has_upstream_branch(repo)? {
        run_git_command(repo, &["push"], "git push failed")?;
    } else {
        run_git_command(
            repo,
            &["push", "--set-upstream", "origin", &branch],
            "git push failed",
        )?;
    }

    println!(
        "  {}  {}",
        ansi::success_bold(Symbol::Success.as_str()),
        ansi::success_bold("Push completed")
    );
    println!(
        "     {}",
        ansi::muted(&format!("origin/{branch} is now updated"))
    );
    println!();
    Ok(())
}

fn run_git_command(repo: &Path, args: &[&str], failure_message: &str) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .with_context(|| format!("failed to run git {}", args.join(" ")))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "git command exited unsuccessfully".to_owned()
    };

    anyhow::bail!("{failure_message}: {detail}");
}

fn has_upstream_branch(repo: &Path) -> Result<bool> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"])
        .current_dir(repo)
        .output()
        .with_context(|| format!("failed to inspect upstream for {}", repo.display()))?;
    Ok(output.status.success())
}

// ─── Commit Lock (Prevent Overlapping Commits) ──────────────────────────

pub struct CommitLock {
    lock_path: PathBuf,
}

impl CommitLock {
    pub fn new(repo: &Path) -> Self {
        Self {
            lock_path: repo.join(".git").join("sapphire-commit.lock"),
        }
    }

    pub fn acquire_with_timeout(&self, timeout_secs: u64) -> Option<Self> {
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        loop {
            if self.try_acquire() {
                return Some(CommitLock {
                    lock_path: self.lock_path.clone(),
                });
            }

            if start.elapsed() >= timeout {
                return None;
            }

            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn try_acquire(&self) -> bool {
        if let Ok(metadata) = fs::metadata(&self.lock_path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    if elapsed < Duration::from_secs(30) {
                        return false;
                    }
                    let _ = fs::remove_file(&self.lock_path);
                }
            }
        }

        fs::write(&self.lock_path, format!("{}", std::process::id())).is_ok()
    }
}

impl Drop for CommitLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

// ─── Git State Helpers for Agents ───────────────────────────────────────

pub fn is_git_dirty(repo: &Path) -> bool {
    let output = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(repo)
        .output();

    match output {
        Ok(o) => !o.stdout.is_empty(),
        Err(_) => false,
    }
}

pub fn is_git_tree_broken(repo: &Path) -> bool {
    let status = Command::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(repo)
        .status();

    match status {
        Ok(s) => !s.success(),
        Err(_) => true,
    }
}

pub fn current_branch(repo: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("branch")
        .arg("--show-current")
        .current_dir(repo)
        .output();

    output
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn git_binary_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn rule_line() -> String {
    "─".repeat(content_width())
}

fn content_width() -> usize {
    terminal_width()
        .map(|width| width.saturating_sub(10).clamp(44, 72))
        .unwrap_or(56)
}

fn render_git_context(git_state: &GitState) {
    let (icon, title, body_lines) = match git_state {
        GitState::Unavailable => (
            ansi::danger_bold(Symbol::Error.as_str()),
            ansi::text_bold("Git CLI Not Available"),
            [
                "Sapphire could not find `git` in the current environment.",
                "Remote setup is unavailable until Git is installed.",
            ],
        ),
        GitState::NotARepo => (
            ansi::brand_soft_bold(Symbol::Warning.as_str()),
            ansi::text_bold("Git Repository Not Initialized"),
            [
                "This directory is not a Git repository yet.",
                "Initialize Git and attach a remote so agents can collaborate cleanly.",
            ],
        ),
        GitState::NoRemote => (
            ansi::brand_soft_bold(Symbol::Warning.as_str()),
            ansi::text_bold("No Git Remote Detected"),
            [
                "This repository does not have a remote configured yet.",
                "Agents can work locally, but push and collaboration flows stay limited until one is configured.",
            ],
        ),
        GitState::Ready { .. } => return,
    };

    println!("  {}", ansi::rule(&rule_line()));
    println!("  {}  {}", icon, title);
    println!("  {}", ansi::rule(&rule_line()));
    println!();
    for line in body_lines {
        println!("  {}", ansi::muted(line));
    }
    println!();
}

fn read_input(label: &str, suffix: &str) -> Result<String> {
    print!("  {} ", ansi::brand_bold(label));
    print!("{}", ansi::muted(suffix));
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_owned())
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_git_url_ssh() {
        assert!(looks_like_git_url("git@github.com:user/repo.git"));
    }

    #[test]
    fn looks_like_git_url_https() {
        assert!(looks_like_git_url("https://github.com/user/repo.git"));
    }

    #[test]
    fn looks_like_git_url_not_a_url() {
        assert!(!looks_like_git_url("hello world"));
    }

    #[test]
    fn looks_like_git_url_gitlab() {
        assert!(looks_like_git_url("git@gitlab.com:group/project.git"));
    }

    #[test]
    fn looks_like_git_url_bitbucket() {
        assert!(looks_like_git_url("https://bitbucket.org/team/repo.git"));
    }
}
