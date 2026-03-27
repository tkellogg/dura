use git2::{BranchType, Diff, DiffOptions, Error, ErrorCode, IndexAddOption, Repository, Signature};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;

use crate::config::Config;

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CaptureStatus {
    pub dura_branch: String,
    pub commit_hash: String,
    pub base_hash: String,
}

impl fmt::Display for CaptureStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "dura: {}, commit_hash: {}, base: {}",
            self.dura_branch, self.commit_hash, self.base_hash
        )
    }
}

pub fn is_repo(path: &Path) -> bool {
    Repository::open(path).is_ok()
}

pub fn capture(path: &Path) -> Result<Option<CaptureStatus>, Error> {
    let repo = Repository::open(path)?;
    let head = match repo.head() {
        Ok(reference) => Some(reference.peel_to_commit()?),
        Err(e) if e.code() == ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e),
    };
    // status check
    if repo.statuses(None)?.is_empty() {
        return Ok(None);
    }

    let branch_name = match &head {
        Some(commit) => format!("dura/{}", commit.id()),
        None => "dura/unborn".to_string(),
    };

    let branch_commit = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(mut branch) => {
            match branch.get().peel_to_commit() {
                Ok(commit) => {
                    // For normal repos: if the branch commit equals head, dura hasn't
                    // made any backup yet — clean up and start fresh.
                    // For unborn repos: any existing commit is a valid prior backup.
                    let dominated_by_head = head.as_ref().is_some_and(|h| commit.id() == h.id());
                    if dominated_by_head {
                        branch.delete()?;
                        None
                    } else {
                        Some(commit)
                    }
                }
                _ => {
                    // Dura branch exists but can't resolve to commit — clean up
                    branch.delete()?;
                    None
                }
            }
        }
        Err(_) => None,
    };

    // Parent is either an existing dura branch commit or the head commit.
    // For unborn repos with no prior dura backup, there is no parent.
    let parent = branch_commit.as_ref().or(head.as_ref());

    // tree
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;

    let old_tree = match parent {
        Some(commit) => Some(commit.tree()?),
        None => None,
    };
    let dirty_diff = repo.diff_tree_to_index(
        old_tree.as_ref(),
        Some(&index),
        Some(DiffOptions::new().include_untracked(true)),
    )?;
    if dirty_diff.deltas().len() == 0 {
        return Ok(None);
    }

    // Build informative commit message with file names and change stats
    let message = build_commit_message(&dirty_diff);

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    // Create dura branch if it doesn't exist.
    // For unborn repos we skip this — repo.commit() with update_ref will create the ref.
    if repo.find_branch(&branch_name, BranchType::Local).is_err() {
        if let Some(head_commit) = &head {
            repo.branch(branch_name.as_str(), head_commit, false)?;
        }
    }

    let committer = Signature::now(&get_git_author(&repo), &get_git_email(&repo))?;
    let parents: Vec<&git2::Commit> = parent.into_iter().collect();
    let oid = repo.commit(
        Some(&format!("refs/heads/{}", &branch_name)),
        &committer,
        &committer,
        &message,
        &tree,
        &parents,
    )?;

    let base_hash = head
        .as_ref()
        .map_or("unborn".to_string(), |h| h.id().to_string());

    Ok(Some(CaptureStatus {
        dura_branch: branch_name,
        commit_hash: oid.to_string(),
        base_hash,
    }))
}

/// Build an informative commit message from diff stats.
///
/// Format: `dura: 3 files (+45/-12) model.py, config.rs, utils.rs`
///
/// The first line stays under 72 chars for git log readability.
/// File names are base names only (no directories) to save space.
fn build_commit_message(diff: &Diff) -> String {
    let stats = match diff.stats() {
        Ok(s) => s,
        Err(_) => return "dura auto-backup".to_string(),
    };

    let files_changed = stats.files_changed();
    let insertions = stats.insertions();
    let deletions = stats.deletions();

    // Collect base file names from deltas
    let file_names: Vec<String> = diff
        .deltas()
        .filter_map(|delta| {
            delta
                .new_file()
                .path()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().into_owned())
        })
        .collect();

    // Build first line: "dura: 3 files (+45/-12) model.py, config.rs"
    let header = format!(
        "dura: {} file{} (+{}/-{})",
        files_changed,
        if files_changed == 1 { "" } else { "s" },
        insertions,
        deletions
    );

    if file_names.is_empty() {
        return header;
    }

    // Append file names, truncating to keep first line under 72 chars
    let mut first_line = format!("{} {}", header, file_names[0]);
    for name in &file_names[1..] {
        let candidate = format!("{}, {}", first_line, name);
        if candidate.len() > 72 {
            first_line = format!("{}...", first_line);
            break;
        }
        first_line = candidate;
    }

    first_line
}

fn get_git_author(repo: &Repository) -> String {
    let dura_cfg = Config::load();
    if let Some(value) = dura_cfg.commit_author {
        return value;
    }

    if !dura_cfg.commit_exclude_git_config {
        if let Ok(git_cfg) = repo.config() {
            if let Ok(value) = git_cfg.get_string("user.name") {
                return value;
            }
        }
    }

    "dura".to_string()
}

fn get_git_email(repo: &Repository) -> String {
    let dura_cfg = Config::load();
    if let Some(value) = dura_cfg.commit_email {
        return value;
    }

    if !dura_cfg.commit_exclude_git_config {
        if let Ok(git_cfg) = repo.config() {
            if let Ok(value) = git_cfg.get_string("user.email") {
                return value;
            }
        }
    }

    "dura@github.io".to_string()
}
