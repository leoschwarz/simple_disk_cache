extern crate simple_disk_cache;
extern crate tempdir;

use simple_disk_cache::SimpleCache;
use simple_disk_cache::config::{CacheConfig, CacheStrategy, DataEncoding};
use tempdir::TempDir;

/// For testing purposes `u32` and `u64` are used because then
/// Deserialize and Serialize are already implemented. In practice
/// you can use any type satisfying the relevant trait bounds.
type TestCache = SimpleCache<u32, u64>;

fn get_tempdir(prefix: &'static str) -> TempDir {
    TempDir::new(format!("sdc_test_{}", prefix).as_str()).expect("failed setting up temp directory")
}

fn basic_usage(encoding: DataEncoding) {
    let tempdir = get_tempdir("basic_usage");
    let config = CacheConfig {
        // This should never be reached in this test.
        max_bytes: 10 * 1024 * 1024,
        encoding,
        strategy: CacheStrategy::LRU,
        subdirs_per_level: 3,
    };
    let mut cache =
        TestCache::initialize(tempdir.as_ref(), config).expect("failed initializing cache.");

    // Insert {5->10, 6->12, …, 20->40}.
    for k in 5..41 {
        let v = (k * 2) as u64;
        cache.put(&k, &v).expect("failed writing to cache.");
    }

    // Retrieve and check the values.
    for k in 5..41 {
        let v_expected = (k * 2) as u64;
        let v = cache.get(&k).expect("failed reading from cache.");
        assert_eq!(v, Some(v_expected));
    }
}

#[test]
fn basic_usage_json() {
    basic_usage(DataEncoding::Json)
}

#[test]
fn basic_usage_bincode() {
    basic_usage(DataEncoding::Bincode)
}

/// Tests writing the cache and then restoring it again.
fn restore_cache(encoding: DataEncoding) {
    let tempdir = get_tempdir("restore_cache");
    let config1 = CacheConfig {
        // This should never be reached in this test.
        max_bytes: 10 * 1024 * 1024,
        encoding,
        strategy: CacheStrategy::LRU,
        subdirs_per_level: 3,
    };
    let config2 = config1.clone();
    let mut cache =
        TestCache::initialize(tempdir.as_ref(), config1).expect("failed initializing cache.");

    // Insert {5->10, 6->12, …, 20->40}.
    for k in 5..41 {
        let v = (k * 2) as u64;
        cache.put(&k, &v).expect("failed writing to cache.");
    }

    drop(cache);
    let mut cache =
        TestCache::initialize(tempdir.as_ref(), config2).expect("failed initializing cache.");

    // Retrieve and check the values.
    for k in 5..41 {
        let v_expected = (k * 2) as u64;
        let v = cache.get(&k).expect("failed reading from cache.");
        assert_eq!(v, Some(v_expected));
    }
}

#[test]
fn restore_cache_json() {
    restore_cache(DataEncoding::Json)
}

#[test]
fn restore_cache_bincode() {
    restore_cache(DataEncoding::Bincode)
}
