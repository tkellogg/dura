use std::path::Path;
use git2::{Repository, Error, IndexAddOption, Oid, Commit, BranchType};

pub fn capture(path: &Path) -> Result<Option<Oid>, Error> {
    let repo = Repository::open(path)?;
    let head = repo.head()?.peel_to_commit()?;
    println!("HEAD: {}", head.id());
    let message = "test commit";

    // status check
    if repo.statuses(None)?.is_empty() {
        return Ok(None);
    }

    // tree
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let branch_name = format!("dura-{}", head.id());
    let branch_commit = find_head(&repo, branch_name.as_str());

    if let Err(_) = repo.find_branch(branch_name.as_str(), BranchType::Local) {
        println!("Branch didn't exist, creating {}", branch_name.as_str());
        repo.branch(branch_name.as_str(), &head, false)?;
        println!("Created.");
    }

    let oid = repo.commit(
        Some(format!("refs/heads/{}", branch_name.as_str()).as_str()),
        &head.author(), 
        &head.committer(),
        message,
        &tree,
        &[ branch_commit.as_ref().unwrap_or(&head) ],
    )?;

    Ok(Some(oid))
}

fn find_head<'repo>(repo: &'repo Repository, branch_name: &str) -> Option<Commit<'repo>> {
    if let Ok(branch) = repo.find_branch(branch_name, BranchType::Local) {
        branch.get().peel_to_commit().ok()
    } else {
        None
    }
}

