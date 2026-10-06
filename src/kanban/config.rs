#![allow(dead_code)]

use crate::board::column::Column;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const DEFAULT_COLUMNS: &[&str] = &["backlog", "todo", "in_progress", "review", "done"];
pub const KANBAN_DIR: &str = ".kanban";
pub const DATABASE_NAME: &str = "kanban.db";
pub const CARDS_DIR: &str = "cards";
pub const COLUMNS_DIR: &str = "columns";

pub struct BoardConfig {
    pub columns: Vec<Column>,
}

impl BoardConfig {
    pub fn new(board_id: &str) -> Self {
        Self {
            columns: Column::default_columns(board_id),
        }
    }
}

/// Get the `.kanban/` directory path for a project.
pub fn kanban_dir(project_path: &Path) -> PathBuf {
    project_path.join(KANBAN_DIR)
}

/// Get the database path for a project.
pub fn db_path(project_path: &Path) -> PathBuf {
    kanban_dir(project_path).join(DATABASE_NAME)
}

/// Get the cards directory path for a project.
pub fn cards_dir(project_path: &Path) -> PathBuf {
    kanban_dir(project_path).join(CARDS_DIR)
}

/// Get the columns directory path for a project.
pub fn columns_dir(project_path: &Path) -> PathBuf {
    kanban_dir(project_path).join(COLUMNS_DIR)
}

/// Check if a project has an initialized kanban board.
pub fn is_initialized(project_path: &Path) -> bool {
    kanban_dir(project_path).exists()
}

/// Resolve the path that owns kanban state for a project checkout.
///
/// Linked git worktrees share the main checkout's board by default. Set
/// `KANBAN_WORKTREE_LOCAL=1` to keep a worktree's board local to that checkout.
pub fn board_project_path(project_path: &Path) -> Result<PathBuf> {
    let project_path = fs::canonicalize(project_path)
        .with_context(|| format!("Cannot resolve project path: {}", project_path.display()))?;

    if worktree_local_enabled() {
        return Ok(project_path);
    }

    Ok(git_common_project_path(&project_path).unwrap_or(project_path))
}

fn worktree_local_enabled() -> bool {
    std::env::var("KANBAN_WORKTREE_LOCAL").is_ok_and(|value| {
        let value = value.trim().to_ascii_lowercase();
        !matches!(value.as_str(), "" | "0" | "false" | "no" | "off")
    })
}

fn git_common_project_path(project_path: &Path) -> Option<PathBuf> {
    let top_level = git_output(project_path, &["rev-parse", "--show-toplevel"])?;
    if PathBuf::from(top_level) != project_path {
        return None;
    }

    let common_dir = PathBuf::from(git_output(
        project_path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?);
    if common_dir.file_name().is_some_and(|name| name == ".git") {
        common_dir.parent().map(Path::to_path_buf)
    } else {
        None
    }
}

fn git_output(project_path: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_path)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct TempRepo {
        root: PathBuf,
    }

    impl TempRepo {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!("kanban_{name}_{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn linked_worktree(&self) -> PathBuf {
            self.root.with_file_name(format!(
                "{}_linked",
                self.root.file_name().unwrap().to_string_lossy()
            ))
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
            let _ = fs::remove_dir_all(self.linked_worktree());
        }
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed\nstdout: {}\nstderr: {}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn init_repo_with_worktree(name: &str) -> (TempRepo, PathBuf) {
        let repo = TempRepo::new(name);
        run_git(repo.path(), &["init"]);
        run_git(repo.path(), &["config", "user.email", "test@example.com"]);
        run_git(repo.path(), &["config", "user.name", "Test User"]);
        fs::write(repo.path().join("README.md"), "test\n").unwrap();
        run_git(repo.path(), &["add", "README.md"]);
        run_git(repo.path(), &["commit", "-m", "initial"]);

        let worktree = repo.linked_worktree();
        let worktree_arg = worktree.to_string_lossy().to_string();
        run_git(
            repo.path(),
            &["worktree", "add", &worktree_arg, "-b", "feature/test"],
        );
        (repo, worktree)
    }

    #[test]
    fn board_project_path_uses_main_checkout_for_linked_worktree_by_default() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("KANBAN_WORKTREE_LOCAL");
        let (repo, worktree) = init_repo_with_worktree("shared_worktree");

        let resolved = board_project_path(&worktree).unwrap();

        assert_eq!(resolved, fs::canonicalize(repo.path()).unwrap());
    }

    #[test]
    fn board_project_path_can_keep_linked_worktree_local_with_env_escape_hatch() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("KANBAN_WORKTREE_LOCAL", "1");
        let (_repo, worktree) = init_repo_with_worktree("local_worktree");

        let resolved = board_project_path(&worktree).unwrap();

        assert_eq!(resolved, fs::canonicalize(&worktree).unwrap());
        std::env::remove_var("KANBAN_WORKTREE_LOCAL");
    }
}
