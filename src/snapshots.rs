use std::path::Path;
use git2::{Repository, Error, IndexAddOption, Oid, Commit, BranchType, DiffOptions};

pub fn capture(path: &Path) -> Result<Option<Oid>, Error> {
    let repo = Repository::open(path)?;
    let head = repo.head()?.peel_to_commit()?;
    println!("HEAD: {}", head.id());
    let message = "test commit";

    // status check
    if repo.statuses(None)?.is_empty() {
        return Ok(None);
    }

    let branch_name = format!("dura-{}", head.id());
    let branch_commit = find_head(&repo, branch_name.as_str());

    if let Err(_) = repo.find_branch(&branch_name, BranchType::Local) {
        println!("Branch didn't exist, creating {}", branch_name.as_str());
        repo.branch(branch_name.as_str(), &head, false)?;
        println!("Created.");
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
        println!("Empty diff");
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

    println!("Committed");
    Ok(Some(oid))
}

fn find_head<'repo>(repo: &'repo Repository, branch_name: &str) -> Option<Commit<'repo>> {
    if let Ok(branch) = repo.find_branch(branch_name, BranchType::Local) {
        branch.get().peel_to_commit().ok()
    } else {
        None
    }
}

