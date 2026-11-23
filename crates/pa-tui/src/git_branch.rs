//! The chat top bar's git branch: a cached read of the session directory's
//! HEAD. No `git` process and no `git status`: only the branch name (or
//! `detached`) is shown. The caller refreshes it when a session attaches,
//! when its label changes, and when a turn ends, never on the paint path.

use std::path::{Path, PathBuf};

/// Read the branch name for `cwd` (a directory inside a repository).
///
/// `None` outside a repository; `Some("detached")` for a detached HEAD.
/// Worktrees and submodules are followed through their `.git` file.
#[must_use]
pub(crate) fn read_branch(cwd: &Path) -> Option<String> {
    let git_dir = find_git_dir(cwd)?;
    let head = std::fs::read_to_string(git_dir.join("HEAD")).ok()?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref: ") {
        let branch = reference.strip_prefix("refs/heads/")?;
        return (!branch.is_empty()).then(|| branch.to_string());
    }
    if head.len() >= 7 && head.chars().all(|c| c.is_ascii_hexdigit()) {
        return Some("detached".to_string());
    }
    None
}

/// The nearest `.git` marker at or above `cwd`: a directory, or the
/// `gitdir:` target a worktree/submodule `.git` file names.
fn find_git_dir(cwd: &Path) -> Option<PathBuf> {
    for dir in cwd.ancestors() {
        let dot_git = dir.join(".git");
        if dot_git.is_dir() {
            return Some(dot_git);
        }
        if dot_git.is_file() {
            if let Some(git_dir) = parse_gitdir(&dot_git, dir) {
                return Some(git_dir);
            }
        }
    }
    None
}

fn parse_gitdir(file: &Path, parent: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(file).ok()?;
    let target = text
        .lines()
        .find_map(|line| line.strip_prefix("gitdir:"))?
        .trim();
    if target.is_empty() {
        return None;
    }
    let path = Path::new(target);
    Some(if path.is_absolute() {
        path.to_path_buf()
    } else {
        parent.join(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        dir
    }

    #[test]
    fn reads_the_branch_from_a_nested_directory() {
        let dir = repo();
        let nested = dir.path().join("crates/pa-tui");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(read_branch(&nested).as_deref(), Some("main"));
        assert_eq!(read_branch(dir.path()).as_deref(), Some("main"));
    }

    #[test]
    fn reads_a_worktree_branch_through_the_gitdir_file() {
        let root = tempfile::tempdir().unwrap();
        let worktree = root.path().join("worktree");
        let git_dir = root.path().join("admin");
        fs::create_dir_all(&worktree).unwrap();
        fs::create_dir_all(&git_dir).unwrap();
        fs::write(git_dir.join("HEAD"), "ref: refs/heads/feature/x\n").unwrap();
        fs::write(
            worktree.join(".git"),
            format!("gitdir: {}\n", git_dir.display()),
        )
        .unwrap();
        assert_eq!(read_branch(&worktree).as_deref(), Some("feature/x"));
    }

    #[test]
    fn a_detached_head_reads_as_detached() {
        let dir = repo();
        fs::write(
            dir.path().join(".git/HEAD"),
            "0123456789abcdef0123456789abcdef01234567\n",
        )
        .unwrap();
        assert_eq!(read_branch(dir.path()).as_deref(), Some("detached"));
    }

    #[test]
    fn outside_a_repository_reads_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(read_branch(dir.path()), None);
    }
}
