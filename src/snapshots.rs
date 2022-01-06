use std::fmt;
use std::path::Path;
use git2::{Repository, Error, IndexAddOption, Commit, BranchType, DiffOptions};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct CaptureStatus {
    pub dura_branch: String,
    pub commit_hash: String,
    pub base_hash: String,
}

impl fmt::Display for CaptureStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "dura: {}, commit_hash: {}, base: {}", self.dura_branch, self.commit_hash, self.base_hash)
    }
}

pub fn capture(path: &Path) -> Result<Option<CaptureStatus>, Error> {
    let repo = Repository::open(path)?;
    let head = repo.head()?.peel_to_commit()?;
    let message = "dura auto-backup";

    // status check
    if repo.statuses(None)?.is_empty() {
        return Ok(None);
    }

    let branch_name = format!("dura-{}", head.id());
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
        Some(DiffOptions::new().include_untracked(true))
    )?;
    if dirty_diff.deltas().len() == 0 {
        return Ok(None)
    }

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let oid = repo.commit(
        Some(format!("refs/heads/{}", branch_name.as_str()).as_str()),
        &head.author(), 
        &head.committer(),
        message,
        &tree,
        &[ branch_commit.as_ref().unwrap_or(&head) ],
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

