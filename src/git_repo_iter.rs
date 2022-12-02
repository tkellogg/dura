use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::btree_map;
use std::fs;
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use os_str_bytes::OsStringBytes;
use qp_trie::Trie;
use num_traits::cast::ToPrimitive;
use rand::prelude::ThreadRng;
use rand::{Rng, thread_rng};
use tracing::{debug, warn};

use crate::config::{Config, WatchConfig};
use crate::snapshots;

/// Internal structure to facilitate "recursion" without blowing up the stack. Without this, we
/// could call self.next() recursively whenever there was an I/O error or when we reached the end
/// of a directory listing. There's no stack space used because we just mutate GitRepoIter, so
/// might as well turn it into a loop.
enum CallState {
    Yield(PathBuf),
    Recurse,
    Done,
}

/// Iterator over all Git repos covered by a config.
///
/// The process is naturally recursive, traversing a directory structure, which made it a poor fit
/// for a more typical filter/map chain.
///
/// Function recursion is used in a few cases:
///  1. Errors: If we get an I/O error, we'll call self.next() again
///  2. Empty iterator: If we get to the end of a sub-iterator, pop & start from the top
///
pub struct GitRepoIter<'a> {
    config_iter: btree_map::Iter<'a, String, Rc<WatchConfig>>,
    /// A stack, because we can't use recursion with an iterator (at least not between elements)
    sub_iter: Vec<(Rc<PathBuf>, Rc<WatchConfig>, CachedDirIter)>,

    cached_fs: Rc<RefCell<CachedFs>>,
}

impl<'a> GitRepoIter<'a> {
    pub fn new(config: &'a Config, cached_fs: Rc<RefCell<CachedFs>>) -> Self {
        (*cached_fs).borrow_mut().cycle();
        Self {
            config_iter: config.repos.iter(),
            sub_iter: Vec::new(),
            cached_fs,
        }
    }

    fn get_next(&mut self) -> CallState {
        // pop
        //
        // Use pop here to manage the lifetime of the iterator. If we used last/peek, we would
        // borrow a shared reference, which precludes us from borrowing as mutable when we want to
        // use the iterator. But that means we have to return it to the vec.
        match self.sub_iter.pop() {
            Some((base_path, watch_config, mut dir_iter)) => {
                let mut next_next: Option<(Rc<PathBuf>, Rc<WatchConfig>, CachedDirIter)> = None;
                let mut ret_val = CallState::Recurse;
                let max_depth: usize = watch_config.max_depth.into();
                if let Some(child_path) = dir_iter.next() {
                    if Self::is_valid_directory(base_path.as_path(), child_path.as_path(), &watch_config)
                    {
                        if snapshots::is_repo(child_path.as_path()) {
                            ret_val = CallState::Yield((*child_path).to_path_buf());
                        } else if self.sub_iter.len() < max_depth {
                            let child_dir_iter = (*self.cached_fs).borrow().list_dir(child_path.to_path_buf());
                            next_next = Some((
                                Rc::clone(&base_path),
                                Rc::clone(&watch_config),
                                child_dir_iter,
                            ));
                        }
                    }
                    // un-pop
                    self.sub_iter
                        .push((Rc::clone(&base_path), Rc::clone(&watch_config), dir_iter));
                }
                if let Some(tuple) = next_next {
                    // directory recursion
                    self.sub_iter.push(tuple);
                }
                ret_val
            }
            None => {
                // Finished dir, queue up next hashmap pair
                match self.config_iter.next() {
                    Some((base_path, watch_config)) => {
                        let path = PathBuf::from(base_path);
                        let dir_iter_opt = path.parent().map(|p| (*self.cached_fs).borrow_mut().list_dir(p.to_path_buf()));
                        if let Some(dir_iter) = dir_iter_opt {
                            // clone because we're going from more global to less global scope
                            self.sub_iter
                                .push((Rc::new(path), Rc::clone(watch_config), dir_iter));
                        }
                        CallState::Recurse
                    }
                    // The end. The real end. This is it.
                    None => CallState::Done,
                }
            }
        }
    }

    /// Checks the provided `child_path` is a directory.
    /// If either `includes` or `excludes` are set,
    /// checks whether the path is included/excluded respectively.
    fn is_valid_directory(base_path: &Path, child_path: &Path, value: &WatchConfig) -> bool {
        if !child_path.is_dir() {
            return false;
        }

        if !child_path.starts_with(base_path) {
            return false;
        }

        let includes = &value.include;
        let excludes = &value.exclude;

        let mut include = true;

        if !excludes.is_empty() {
            include = !excludes
                .iter()
                .any(|exclude| child_path.starts_with(base_path.join(exclude)));
        }

        if !include && !includes.is_empty() {
            include = includes
                .iter()
                .any(|include| base_path.join(include).starts_with(child_path));
        }

        include
    }
}

impl<'a> Iterator for GitRepoIter<'a> {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.get_next() {
                CallState::Yield(path) => return Some(path),
                CallState::Recurse => continue,
                CallState::Done => return None,
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CacheItem {
    /// Random number used for occasionally invalidating the cache
    sig_invalidate: u8,
    /// Force invalidate at this point
    ttl: Instant,
    initialized: bool,
}

/// A repository of directory iterators that caches to avoid hitting the disk. Cache
/// invalidation is done with lots of jitter, so that items are given a maximum lifetime,
/// i.e. cache invalidation is guaranteed to occur every N minutes, but in practice
/// invalidation is spread evenly, stochastically, over those N minutes. The intent is
/// to avoid a single spike of sys calls to list all directories.
///
/// Without this optimization, it spends a lot of time listing directories, but most
/// computers can handle it fine.
///
/// This makes sense for the GitRepoIter because the set of repos is fairly static and
/// the cost of being wrong for a few minutes is very low. This doesn't make sense for
/// scanning the files in the repo, because the cost of a false negative is very high.
#[derive(Debug)]
pub struct CachedFs {
    cache: Rc<RefCell<Trie<PPath, CacheItem>>>,
    max_lifetime: Duration,
    max_sig_ticks: u8,
    current_sig_tick: u8,
    rng: Rc<RefCell<ThreadRng>>,
}

impl CachedFs {
    pub fn new(max_lifetime: Duration, expected_interval: Duration) -> Self {
        let max_sig_ticks = (max_lifetime.as_secs_f32() / expected_interval.as_secs_f32()).to_u8();
        let mut rng = thread_rng();
        let max_ticks = max_sig_ticks.unwrap_or(255);
        Self {
            cache: Rc::new(RefCell::new(Trie::new())),
            max_lifetime,
            max_sig_ticks: max_ticks,
            current_sig_tick: rng.gen_range(0u8 .. max_ticks),
            rng: Rc::new(RefCell::new(rng)),
        }
    }

    pub fn cycle(&mut self) {
        self.current_sig_tick = self.gen_rand();
    }

    fn gen_rand(&mut self) -> u8 {
        self.rng.borrow_mut().gen_range(0u8 .. self.max_sig_ticks)
    }

    pub fn list_dir<'a>(&self, path: PathBuf) -> CachedDirIter {
        let ppath = PPath::new(&path);
        let cache = (*self.cache).borrow();
        match cache.get::<PPath>(&ppath) {
            Some(found) if found.sig_invalidate == self.current_sig_tick => {
                self.send_miss(&ppath)
            }
            Some(found) if !found.initialized => {
                self.send_miss(&ppath)
            }
            Some(_) => {
                self.send_hit(&ppath)
            }
            None => {
                self.send_miss(&ppath)
            }
        }
    }

    fn send_hit(&self, ppath: &PPath) -> CachedDirIter {
        let copy: Vec<_> = (*self.cache).borrow().iter_prefix::<PPath>(&ppath)
            // TODO: do we have to remove non-direct children? Write tests and find out...
            .map(|pair| {
                let (ppath, _) = pair;
                Rc::new(ppath.path())
            })
            .collect();

        CachedDirIter::Hit {
            index: 0,
            listing: copy,
        }
    }

    fn send_miss(&self, ppath: &PPath) -> CachedDirIter {
        // create a NewCacheItem function
        let copied_rng = Rc::clone(&self.rng);
        let ttl = Instant::now().add(self.max_lifetime);
        let max_sig_ticks = self.max_sig_ticks;
        let new_cache_item: NewCacheItem = Rc::new(move || CacheItem {
            sig_invalidate: copied_rng.borrow_mut().gen_range(0u8 .. max_sig_ticks),
            ttl,
            initialized: false,
        });

        // read dir
        match fs::read_dir((&ppath.path()).as_path()) {
            Ok(iter) => CachedDirIter::Miss(
                iter,
                Rc::clone(&self.cache),
                Rc::clone(&new_cache_item),
            ),
            Err(err) => {
                warn!("Failed to read dir: {} due to error: {}", &ppath.path().display(), err);
                // an empty hit
                CachedDirIter::Hit {index: 0, listing: vec![] }
            },
        }
    }
}

impl Default for CachedFs {
    fn default() -> Self {
        Self {
            cache: Rc::new(RefCell::new(Trie::new())),
            max_lifetime: Duration::from_secs_f64(600.0),
            max_sig_ticks: 60u8,
            current_sig_tick: 0u8,
            rng: Rc::new(RefCell::new(thread_rng()))
            // rng: Rc<RefCell<ThreadRng>>,
        }
    }
}

// just to support the trie + type checker
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PPath {
    bytes: Vec<u8>
}

impl PPath {
    pub fn new(pb: &PathBuf) -> Self {
        Self {
            bytes: pb.to_path_buf().into_raw_vec()
        }
    }

    pub fn path(&self) -> PathBuf {
        PathBuf::assert_from_raw_vec(self.bytes.clone())
    }
}

impl Borrow<[u8]> for PPath {
    fn borrow(&self) -> &[u8] {
        &*self.bytes
    }
}

type NewCacheItem = Rc<dyn Fn() -> CacheItem>;

/// Union over 2 types of iterators over directories. Either the
/// cache hit or cache miss mode.
pub enum CachedDirIter {
    Miss(fs::ReadDir, Rc<RefCell<Trie<PPath, CacheItem>>>, NewCacheItem),
    Hit { index: usize, listing: Vec<Rc<PathBuf>>}
}

impl Iterator for CachedDirIter {
    type Item = Rc<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            CachedDirIter::Miss(iter, cache_cell, new_cache_item) => {
                iter.next().and_then(|e|
                    e.ok().map(|d| {
                        let path = Rc::new(d.path());
                        let res = (**cache_cell).borrow_mut().insert(
                            PPath::new(&path),
                            new_cache_item(),
                        );

                        match res {
                            Some(_) => {
                                debug!("Updated cache for directory; path={path}",
                                    path=(*path).display());
                            }
                            None => {
                                debug!("Created item in cache for directory; path={path}",
                                    path=(*path).display());
                            }
                        }
                        path
                    })
                )
            }
            CachedDirIter::Hit { ref mut index, listing } => {
                let result = if *index >= listing.len() {
                    None
                } else {
                    Some(Rc::clone(&listing[*index]))
                };
                *index += 1;
                result
            }
        }
    }
}
