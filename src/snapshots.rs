use std::path::Path;
use git2::{Repository, Error, IndexAddOption, Oid};

pub fn create(path: &Path) -> Result<Option<Oid>, Error> {
    let repo = Repository::open(path)?;
    let head = repo.head()?.peel_to_commit()?;
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

    let oid = repo.commit(
        None, // update_ref: we'll do this a different way
        &head.author(), 
        &head.committer(),
        message,
        &tree,
        &vec![ &head ],
    )?;

    Ok(Some(oid))
}

