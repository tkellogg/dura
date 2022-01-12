mod util;

use dura::config::Config;
use std::fs;
use dura::database::RuntimeLock;

#[test]
fn start_serve() {
    let mut dura = util::dura::Dura::new();
    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_runtime_lock());
    assert_eq!(None, dura.get_runtime_lock());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let runtime_lock = dura.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(dura.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_null_pid_in_config() {
    let mut dura = util::dura::Dura::new();
    let mut runtime_lock = RuntimeLock::empty();
    runtime_lock.pid = None;
    dura.save_runtime_lock(&runtime_lock);

    assert_eq!(None, dura.pid(true));
    assert_ne!(None, dura.get_runtime_lock());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let runtime_lock = dura.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(dura.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_other_pid_in_config() {
    let mut dura = util::dura::Dura::new();
    let mut runtime_lock = RuntimeLock::empty();
    runtime_lock.pid = Some(12345);
    dura.save_runtime_lock(&runtime_lock);

    println!("db:: {:?}", dura.get_runtime_lock());

    assert_eq!(None, dura.pid(true));
    assert_ne!(None, dura.get_runtime_lock());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let runtime_lock = dura.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(dura.pid(true), runtime_lock.unwrap().pid);
}

#[test]
fn start_serve_with_invalid_json() {
    let mut dura = util::dura::Dura::new();
    let runtime_lock_path = dura.runtime_lock_path();
    Config::create_dir(runtime_lock_path.as_path());
    fs::write(
        runtime_lock_path,
        "{\"pid\":34725",
    )
    .unwrap();

    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_runtime_lock());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let runtime_lock = dura.get_runtime_lock();
    assert_ne!(None, runtime_lock);
    assert_eq!(dura.pid(true), runtime_lock.unwrap().pid);
}

