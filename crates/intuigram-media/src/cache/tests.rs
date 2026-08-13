use std::time::Duration;
use std::{fs, thread};

use tempfile::tempdir;

use super::{CacheKey, CacheKind, CacheOwner, MediaCache, entry_from_child};

#[test]
fn cache_is_bounded_across_media_and_thumbnails() {
    let temporary = tempdir().expect("temporary cache root should be created");
    let cache = MediaCache::new(temporary.path().join("7"), 5);
    cache
        .put(CacheKind::Media, &CacheKey::new("old"), b"123")
        .expect("first entry should be cached");
    thread::sleep(Duration::from_millis(10));
    cache
        .put(CacheKind::Thumbnail, &CacheKey::new("new"), b"456")
        .expect("second entry should trigger eviction");

    assert_eq!(cache.usage().expect("usage should be readable").bytes, 3);
    assert_eq!(
        cache
            .get(CacheKind::Media, &CacheKey::new("old"))
            .expect("cache miss should be readable"),
        None
    );
    assert_eq!(
        cache
            .get(CacheKind::Thumbnail, &CacheKey::new("new"))
            .expect("cache hit should be readable"),
        Some(b"456".to_vec())
    );
}

#[test]
fn retained_entries_survive_lru_pressure_until_released() {
    let temporary = tempdir().expect("temporary cache root should be created");
    let cache = MediaCache::new(temporary.path().join("7"), 3);
    let owner = CacheOwner::new("chat:7");
    let retained = CacheKey::new("retained");
    cache
        .put_retained(CacheKind::Media, &owner, &retained, b"protected")
        .expect("protected entry should be cached");
    cache
        .put(CacheKind::Media, &CacheKey::new("ordinary"), b"123")
        .expect("ordinary entry should fit its independent bound");
    assert_eq!(cache.usage().expect("total usage should load").bytes, 12);

    assert_eq!(
        cache
            .get_retained(CacheKind::Media, &owner, &retained)
            .expect("protected entry should be readable"),
        Some(b"protected".to_vec())
    );

    cache
        .release(&owner)
        .expect("owner should become evictable");
    assert_eq!(
        cache
            .get_retained(CacheKind::Media, &owner, &retained)
            .expect("released namespace should be absent"),
        None
    );
    assert!(cache.usage().expect("bounded usage should load").bytes <= 3);
}

#[test]
fn clear_never_reaches_sibling_durable_records() {
    let temporary = tempdir().expect("temporary root should be created");
    let durable = temporary.path().join("data/7.db");
    fs::create_dir_all(durable.parent().expect("fixture has a parent"))
        .expect("durable directory should be created");
    fs::write(&durable, b"message text").expect("durable fixture should be written");
    let cache = MediaCache::new(temporary.path().join("cache/7"), 1024);
    cache
        .put(CacheKind::Media, &CacheKey::new("image"), b"bytes")
        .expect("entry should be cached");

    let removed = cache.clear().expect("cache should clear");

    assert_eq!(removed.bytes, 5);
    assert_eq!(
        fs::read(durable).expect("durable data must remain"),
        b"message text"
    );
    assert_eq!(cache.usage().expect("usage should be readable").bytes, 0);
}

#[test]
fn cache_scan_vanished_partial_remains_available() {
    let temporary = tempdir().expect("temporary cache root should be created");
    let directory = temporary.path().join("thumbnails");
    fs::create_dir(&directory).expect("thumbnail directory should be created");
    let partial = directory.join("2f4ff9b66ff93247.partial");
    fs::write(&partial, b"in flight").expect("partial entry should be written");
    let child = fs::read_dir(&directory)
        .expect("thumbnail directory should be readable")
        .next()
        .expect("partial entry should be listed")
        .expect("partial directory entry should be readable");
    fs::rename(&partial, directory.join("2f4ff9b66ff93247.cache"))
        .expect("writer should atomically install the partial entry");
    assert!(
        entry_from_child(child)
            .expect("transient partial files should not make the cache unavailable")
            .is_none()
    );
}
