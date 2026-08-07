//! Size-bounded, redownloadable media storage.

use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use snafu::{ResultExt, Snafu};

/// Failure while accessing redownloadable cached bytes.
#[derive(Debug, Snafu)]
#[snafu(visibility(pub))]
pub enum Error {
    /// A cache directory could not be created or inspected.
    #[snafu(display("failed to access media cache directory {}", path.display()))]
    Directory {
        path: PathBuf,
        source: std::io::Error,
    },

    /// A cache entry could not be read.
    #[snafu(display("failed to read media cache entry {}", path.display()))]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },

    /// A cache entry could not be written atomically.
    #[snafu(display("failed to write media cache entry {}", path.display()))]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },

    /// A cache entry could not be removed during eviction or an explicit clear.
    #[snafu(display("failed to remove media cache entry {}", path.display()))]
    Remove {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// Result returned by media-cache operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Separate cache families from Intuigram's documented filesystem layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheKind {
    /// Original media bytes suitable for later decoding or saving.
    Media,

    /// Small derived previews.
    Thumbnail,
}

impl CacheKind {
    const fn directory(self) -> &'static str {
        match self {
            Self::Media => "media",
            Self::Thumbnail => "thumbnails",
        }
    }
}

/// Stable opaque identity for one remotely redownloadable object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheKey(String);

impl CacheKey {
    /// Creates a key from adapter-owned remote identity components.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn filename(&self) -> String {
        // FNV-1a is used only to create a filesystem-safe name. The full key is
        // adapter-owned and a collision merely causes a harmless redownload.
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for byte in self.0.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
        format!("{hash:016x}.cache")
    }
}

/// Current bounded-cache accounting.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CacheUsage {
    /// Bytes currently retained across media and thumbnail entries.
    pub bytes: u64,

    /// Number of retained cache entries.
    pub entries: usize,

    /// Configured upper bound in bytes.
    pub limit: u64,
}

/// Per-Account cache containing only redownloadable bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MediaCache {
    root: PathBuf,
    limit: u64,
}

impl MediaCache {
    /// Creates an Account cache. `root` must be the Account-specific cache
    /// directory, never its durable database directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, limit: u64) -> Self {
        Self {
            root: root.into(),
            limit,
        }
    }

    /// Reads and marks an entry as most recently used.
    pub fn get(&self, kind: CacheKind, key: &CacheKey) -> Result<Option<Vec<u8>>> {
        let path = self.path(kind, key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(Error::Read { path, source }),
        };
        // Rewriting identical bytes updates the portable modification timestamp,
        // which is the cache's durable LRU clock.
        fs::write(&path, &bytes).context(WriteSnafu { path: path.clone() })?;
        Ok(Some(bytes))
    }

    /// Atomically retains bytes and evicts least-recently-used entries until
    /// the configured bound is satisfied.
    pub fn put(&self, kind: CacheKind, key: &CacheKey, bytes: &[u8]) -> Result<()> {
        let directory = self.root.join(kind.directory());
        fs::create_dir_all(&directory).context(DirectorySnafu {
            path: directory.clone(),
        })?;
        let path = self.path(kind, key);
        let partial = path.with_extension("partial");
        fs::write(&partial, bytes).context(WriteSnafu {
            path: partial.clone(),
        })?;
        fs::rename(&partial, &path).context(WriteSnafu { path: path.clone() })?;
        self.enforce_limit()
    }

    /// Reports bytes and entries without touching their recency.
    pub fn usage(&self) -> Result<CacheUsage> {
        let entries = self.entries()?;
        Ok(CacheUsage {
            bytes: entries.iter().map(|entry| entry.bytes).sum(),
            entries: entries.len(),
            limit: self.limit,
        })
    }

    /// Removes only redownloadable media and thumbnail bytes.
    pub fn clear(&self) -> Result<CacheUsage> {
        let before = self.usage()?;
        match fs::remove_dir_all(&self.root) {
            Ok(()) => Ok(before),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(before),
            Err(source) => Err(Error::Remove {
                path: self.root.clone(),
                source,
            }),
        }
    }

    fn path(&self, kind: CacheKind, key: &CacheKey) -> PathBuf {
        self.root.join(kind.directory()).join(key.filename())
    }

    fn enforce_limit(&self) -> Result<()> {
        let mut entries = self.entries()?;
        entries.sort_by_key(|entry| (entry.used, entry.path.clone()));
        let mut bytes = entries.iter().map(|entry| entry.bytes).sum::<u64>();
        for entry in entries {
            if bytes <= self.limit {
                break;
            }
            fs::remove_file(&entry.path).context(RemoveSnafu {
                path: entry.path.clone(),
            })?;
            bytes = bytes.saturating_sub(entry.bytes);
        }
        Ok(())
    }

    fn entries(&self) -> Result<Vec<Entry>> {
        let mut entries = Vec::new();
        for kind in [CacheKind::Media, CacheKind::Thumbnail] {
            let directory = self.root.join(kind.directory());
            let children = match fs::read_dir(&directory) {
                Ok(children) => children,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(Error::Directory {
                        path: directory,
                        source,
                    });
                }
            };
            for child in children {
                let child = child.context(DirectorySnafu {
                    path: directory.clone(),
                })?;
                let path = child.path();
                let metadata = child.metadata().context(ReadSnafu { path: path.clone() })?;
                if metadata.is_file() && path.extension().is_some_and(|value| value == "cache") {
                    entries.push(Entry {
                        path,
                        bytes: metadata.len(),
                        used: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    });
                }
            }
        }
        Ok(entries)
    }
}

#[derive(Debug)]
struct Entry {
    path: PathBuf,
    bytes: u64,
    used: SystemTime,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use std::{fs, thread};

    use tempfile::tempdir;

    use super::{CacheKey, CacheKind, MediaCache};

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
}
