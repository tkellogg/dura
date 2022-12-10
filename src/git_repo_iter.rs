use num_traits::abs;
use num_traits::cast::ToPrimitive;
use os_str_bytes::OsStringBytes;
use qp_trie::Trie;
use rand::prelude::ThreadRng;
use rand::{thread_rng, Rng};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::btree_map;
use std::fs;
use std::ops::Add;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};
use tracing::{debug, info, trace, warn};

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
                    if Self::is_valid_directory(
                        base_path.as_path(),
                        child_path.as_path(),
                        &watch_config,
                    ) {
                        if snapshots::is_repo(child_path.as_path()) {
                            ret_val = CallState::Yield((*child_path).to_path_buf());
                        } else if self.sub_iter.len() < max_depth {
                            let child_dir_iter = (*self.cached_fs)
                                .borrow()
                                .list_dir(child_path.to_path_buf());
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
                        let dir_iter_opt = path
                            .parent()
                            .map(|p| (*self.cached_fs).borrow_mut().list_dir(p.to_path_buf()));
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CacheItem {
    /// Random number used for occasionally invalidating the cache
    sig_invalidate: u16,
    /// Force invalidate at this point
    ttl: Instant,
    children: Option<Rc<RefCell<Vec<String>>>>,
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
    max_sig_ticks: u16,
    current_sig_tick: u16,
    cycle_count: u16,
    rng: Rc<RefCell<ThreadRng>>,
    disable: bool,
}

impl CachedFs {
    /// # Arguments
    ///
    /// * `max_lifetime` — The maximum time a cache entry should live. In other words, "how long
    ///   are you willing to go without a new Git repo being discovered?"
    /// * `expected_interval` — The average duration between calls to `list_dir`. i.e. average
    ///   loop time, from the statistics.
    pub fn new(max_lifetime: Duration, expected_interval: Duration) -> Self {
        assert!(
            max_lifetime > expected_interval,
            "max_lifetime should be larger than expected_interval, otherwise
strange behavior may occur"
        );
        let max_sig_ticks =
            // Decided on this after modeling it in ./scripts/CachedFs.ipynb
            // Having a ratio of 1/4 gives it decent behavior
            ((max_lifetime.as_secs_f32() / 4.0) / expected_interval.as_secs_f32())
            .to_u16();
        let mut rng = thread_rng();
        let max_ticks = max_sig_ticks.unwrap_or(255);
        Self {
            cache: Rc::new(RefCell::new(Trie::new())),
            max_lifetime,
            max_sig_ticks: max_ticks,
            current_sig_tick: rng.gen_range(0..max_ticks),
            rng: Rc::new(RefCell::new(rng)),
            cycle_count: 0,
            disable: false,
        }
    }

    pub fn disable_cache(&mut self) {
        self.disable = true;
    }

    /// Used in testing to reset TTL of all the nodes in the trie to the current instant.
    /// Within tests, this makes it so we can keep the time horizons very low. I don't think
    /// it's much use outside tests.
    pub fn reset_ttl(&self) {
        let now = Instant::now().add(self.max_lifetime);
        let mut cache = (*self.cache).borrow_mut();
        for (_, cache_item) in cache.iter_mut() {
            cache_item.ttl = now;
        }
        info!("Reset all TTL items");
    }

    /// This should be called every now and then. If it's not called periodically, our cache
    /// items won't expire. Originally I intended this to be called at the start of processing
    /// all Git repos once. I think it could be called more often than that, certainly not less.
    pub fn cycle(&mut self) {
        self.cycle_count = (self.cycle_count + 1) % (self.max_sig_ticks * 4);
        self.current_sig_tick = self.gen_rand();
    }

    /// Generate a random number.
    /// The notebook in `./scripts/CachedFs.ipynb` explores the functions and coefficients.
    /// I arrived at these magic numbers after a lot of experimentation. It seems that these
    /// tend to generate a distribution that's a little biased toward left.
    fn gen_rand(&mut self) -> u16 {
        let n = self.max_sig_ticks as f32;
        let i = self.cycle_count as f32;
        let max = (n + abs((2.0 * n) - i.powf(0.9)))
            .to_u16()
            .unwrap_or(u16::MAX);
        (*self.rng).borrow_mut().gen_range(0..max)
    }

    /// List the directory. This should be have the same as fs::ReadDir except that
    /// it may return a cached version instead, to avoid disk access.
    pub fn list_dir(&self, path: PathBuf) -> CachedDirIter {
        let ppath = PPath::new(&path);
        let cache_item = {
            let cache = (*self.cache).borrow();
            cache.get::<PPath>(&ppath).cloned()
        };

        match cache_item {
            _ if self.disable => {
                debug!("Cache disabled; path={}", ppath.to_string());
                self.send_miss(&ppath)
            }
            Some(found) if found.sig_invalidate == self.current_sig_tick => {
                debug!(
                    "Cache miss, random bucket; sig_invalidate={}, path={}",
                    found.sig_invalidate,
                    ppath.to_string()
                );
                self.send_miss(&ppath)
            }
            Some(found) if found.children.is_none() => {
                debug!("Cache miss, uninitialized; path={}", ppath.to_string());
                self.send_miss(&ppath)
            }
            Some(found) if found.ttl < Instant::now() => {
                debug!(
                    "Cache miss, timeout; ttl_delta={}, secs, path={}",
                    (Instant::now() - found.ttl).as_secs_f32(),
                    ppath.to_string()
                );
                self.send_miss(&ppath)
            }
            Some(found) => {
                trace!(
                    "Cache hit; path={}, local={}, global={}, num_buckets={}",
                    ppath.to_string(),
                    found.sig_invalidate,
                    self.current_sig_tick,
                    self.max_sig_ticks
                );
                self.send_hit(&ppath)
            }
            None => {
                debug!("Cache miss, not present; path={}", ppath.to_string());
                self.send_miss(&ppath)
            }
        }
    }

    fn send_hit(&self, ppath: &PPath) -> CachedDirIter {
        let empty = Rc::new(RefCell::new(vec![]));
        let cache = (*self.cache).borrow();
        let complex = cache
            .get::<PPath>(ppath)
            .and_then(|item| item.children.as_ref())
            .unwrap_or(&empty);
        let children = (**complex)
            .borrow()
            .iter()
            .map(|str| Rc::new(ppath.path().join(str)))
            .collect::<Vec<_>>();

        CachedDirIter::Hit {
            index: 0,
            listing: children,
        }
    }

    /// Create a NewCacheItem function
    ///
    /// TODO: I've rationalized to myself that NewCacheItem needs to be a function, but maybe it
    /// can be simplified?
    fn get_new_cache_item(&self) -> NewCacheItem {
        let copied_rng = Rc::clone(&self.rng);
        let ttl = Instant::now().add(self.max_lifetime);
        let max_sig_ticks = self.max_sig_ticks;
        let new_cache_item: NewCacheItem = Rc::new(move || CacheItem {
            sig_invalidate: (*copied_rng).borrow_mut().gen_range(0u16..max_sig_ticks),
            ttl,
            children: None,
        });
        new_cache_item
    }

    fn send_miss(&self, ppath: &PPath) -> CachedDirIter {
        let new_cache_item = self.get_new_cache_item();

        // read dir
        match fs::read_dir((ppath.path()).as_path()) {
            Ok(iter) => {
                let item = new_cache_item();
                (*self.cache).borrow_mut().insert(ppath.clone(), item);
                debug!(
                    "Initialized cache for directory; path={path}",
                    path = ppath.to_string()
                );

                CachedDirIter::Miss(iter, Rc::clone(&self.cache), Rc::clone(&new_cache_item))
            }
            Err(err) => {
                warn!(
                    "Failed to read dir: {} due to error: {}",
                    &ppath.path().display(),
                    err
                );
                // an empty hit
                CachedDirIter::Hit {
                    index: 0,
                    listing: vec![],
                }
            }
        }
    }
}

impl Default for CachedFs {
    fn default() -> Self {
        Self::new(Duration::from_secs_f64(600.0), Duration::from_secs_f64(5.0))
    }
}

// just to support the trie + type checker
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PPath {
    bytes: Vec<u8>,
}

impl PPath {
    pub fn new(pb: &Path) -> Self {
        Self {
            bytes: pb.to_path_buf().into_raw_vec(),
        }
    }

    pub fn path(&self) -> PathBuf {
        PathBuf::assert_from_raw_vec(self.bytes.clone())
    }
}

// for debugging purposes
impl ToString for PPath {
    fn to_string(&self) -> String {
        match self.path().into_os_string().into_string() {
            Ok(str) => str,
            Err(_) => "n/a".to_string(),
        }
    }
}

impl Borrow<[u8]> for PPath {
    fn borrow(&self) -> &[u8] {
        &self.bytes
    }
}

type NewCacheItem = Rc<dyn Fn() -> CacheItem>;

/// Union over 2 types of iterators over directories. Either the
/// cache hit or cache miss mode.
pub enum CachedDirIter {
    Miss(
        fs::ReadDir,
        Rc<RefCell<Trie<PPath, CacheItem>>>,
        NewCacheItem,
    ),
    Hit {
        index: usize,
        listing: Vec<Rc<PathBuf>>,
    },
}

impl Iterator for CachedDirIter {
    type Item = Rc<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            CachedDirIter::Miss(iter, cache_cell, new_cache_item) => {
                iter.next().and_then(|e| {
                    e.ok().map(|dir_entry| {
                        let entry_path = Rc::new(dir_entry.path());
                        let res = (**cache_cell)
                            .borrow_mut()
                            .insert(PPath::new(&entry_path), new_cache_item());

                        match res {
                            Some(_) => {
                                trace!(
                                    "Updated cache for directory; path={path}",
                                    path = (*entry_path).display()
                                );
                            }
                            None => {
                                trace!(
                                    "Created item in cache for directory; path={path}",
                                    path = (*entry_path).display()
                                );
                            }
                        }

                        // insert this item into parent node
                        if let Some(parent) = entry_path.parent() {
                            let search_node = PPath::new(parent);
                            if let Some(found_item) =
                                (**cache_cell).borrow_mut().get_mut::<PPath>(&search_node)
                            {
                                // maintain a vec of dir names in self
                                let comp = (*entry_path).components().last().and_then(|last| {
                                    last.as_os_str().to_os_string().into_string().ok()
                                });
                                match &found_item.children {
                                    Some(c) => {
                                        if let Some(last_component) = comp {
                                            (*c).borrow_mut().push(last_component);
                                        }
                                    }
                                    None => {
                                        let mut vec = vec![];
                                        if let Some(last_component) = comp {
                                            vec.push(last_component);
                                        }
                                        found_item.children = Some(Rc::new(RefCell::new(vec)));
                                    }
                                };
                            }
                        }

                        entry_path
                    })
                })
            }
            CachedDirIter::Hit {
                ref mut index,
                listing,
            } => {
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

#[cfg(test)]
mod tests {
    use crate::git_repo_iter::CachedFs;
    use std::time::Duration;

    #[test]
    fn new_cachedfs() {
        let cf = CachedFs::new(Duration::from_secs(120), Duration::from_secs(5));
        assert_eq!(cf.max_sig_ticks, 6);
        assert!(cf.current_sig_tick < cf.max_sig_ticks);
    }

    #[test]
    #[should_panic]
    fn new_cachedfs_panic_when_parameters_reversed() {
        CachedFs::new(Duration::from_secs(5), Duration::from_secs(120));
    }
}
