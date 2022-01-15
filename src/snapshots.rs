use git2::{BranchType, Commit, DiffOptions, Error, IndexAddOption, Repository, Signature, Time};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use std::{fmt, env};
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
    let head = repo.head()?.peel_to_commit()?;
    let message = "dura auto-backup";

    // status check
    if repo.statuses(None)?.is_empty() {
        return Ok(None);
    }

    let branch_name = format!("dura/{}", head.id());
    let branch_commit = find_head(&repo, branch_name.as_str());

    if repo.find_branch(&branch_name, BranchType::Local).is_err() {
        repo.branch(branch_name.as_str(), &head, false)?;
    }

    // tree
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;

    let dirty_diff = repo.diff_tree_to_index(
        Some(&branch_commit.as_ref().unwrap_or(&head).tree()?),
        Some(&index),
        Some(DiffOptions::new().include_untracked(true)),
    )?;
    if dirty_diff.deltas().len() == 0 {
        return Ok(None);
    }

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let committer = match env::var("GIT_COMMITTER_DATE") {
        Err(_) => {
            Signature::now(&get_git_author(&repo), &get_git_email(&repo))?
        }
        Ok(date_str) => {
            let chrono_time = DateTime::parse_from_rfc3339(date_str.as_str())
                .or_else(|_| DateTime::parse_from_rfc2822(date_str.as_str()))
                .unwrap();
            let offset = chrono_time.timezone().local_minus_utc();
            let time = Time::new(chrono_time.timestamp(), offset);
            Signature::new(&get_git_author(&repo), &get_git_email(&repo), &time)?
        }
    };
    let oid = repo.commit(
        Some(format!("refs/heads/{}", branch_name.as_str()).as_str()),
        &committer,
        &committer,
        message,
        &tree,
        &[branch_commit.as_ref().unwrap_or(&head)],
    )?;

    Ok(Some(CaptureStatus {
        dura_branch: branch_name,
        commit_hash: oid.to_string(),
        base_hash: head.id().to_string(),
    }))
}

fn find_head<'repo>(repo: &'repo Repository, branch_name: &str) -> Option<Commit<'repo>> {
    if let Ok(branch) = repo.find_branch(branch_name, BranchType::Local) {
        branch.get().peel_to_commit().ok()
    } else {
        None
    }
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
