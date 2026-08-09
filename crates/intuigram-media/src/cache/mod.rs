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

/// Stable owner of entries protected from ordinary cache eviction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CacheOwner(String);

impl CacheOwner {
    /// Creates an owner from an adapter-owned stable identity.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    fn directory(&self) -> String {
        CacheKey::new(&self.0).filename().replace(".cache", "")
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
        self.read_path(self.path(kind, key))
    }

    /// Atomically retains bytes and evicts least-recently-used entries until
    /// the configured bound is satisfied.
    pub fn put(&self, kind: CacheKind, key: &CacheKey, bytes: &[u8]) -> Result<()> {
        self.write_path(self.path(kind, key), bytes)?;
        self.enforce_limit()
    }

    /// Reads protected bytes without exposing them to ordinary LRU eviction.
    pub fn get_retained(
        &self,
        kind: CacheKind,
        owner: &CacheOwner,
        key: &CacheKey,
    ) -> Result<Option<Vec<u8>>> {
        self.read_path(self.retained_path(kind, owner, key))
    }

    /// Atomically stores bytes in an owner namespace excluded from LRU scans.
    pub fn put_retained(
        &self,
        kind: CacheKind,
        owner: &CacheOwner,
        key: &CacheKey,
        bytes: &[u8],
    ) -> Result<()> {
        let path = self.retained_path(kind, owner, key);
        self.write_path(path, bytes)
    }

    /// Returns one owner's protected entries to ordinary cache eviction.
    pub fn release(&self, owner: &CacheOwner) -> Result<()> {
        for kind in [CacheKind::Media, CacheKind::Thumbnail] {
            let retained = self.retained_directory(kind, owner);
            let children = match fs::read_dir(&retained) {
                Ok(children) => children,
                Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
                Err(source) => {
                    return Err(Error::Directory {
                        path: retained,
                        source,
                    });
                }
            };
            let ordinary = self.root.join(kind.directory());
            fs::create_dir_all(&ordinary).context(DirectorySnafu {
                path: ordinary.clone(),
            })?;
            for child in children {
                let child = child.context(DirectorySnafu {
                    path: retained.clone(),
                })?;
                let source = child.path();
                if !source.is_file() {
                    continue;
                }
                let destination = ordinary.join(child.file_name());
                if destination.is_file() {
                    fs::remove_file(&source).context(RemoveSnafu { path: source })?;
                } else {
                    fs::rename(&source, &destination).context(WriteSnafu { path: destination })?;
                }
            }
            fs::remove_dir_all(&retained).context(RemoveSnafu { path: retained })?;
        }
        self.enforce_limit()
    }

    /// Reports bytes and entries without touching their recency.
    pub fn usage(&self) -> Result<CacheUsage> {
        let entries = self.all_entries()?;
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

    fn retained_directory(&self, kind: CacheKind, owner: &CacheOwner) -> PathBuf {
        self.root
            .join(kind.directory())
            .join("retained")
            .join(owner.directory())
    }

    fn retained_path(&self, kind: CacheKind, owner: &CacheOwner, key: &CacheKey) -> PathBuf {
        self.retained_directory(kind, owner).join(key.filename())
    }

    fn read_path(&self, path: PathBuf) -> Result<Option<Vec<u8>>> {
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

    fn write_path(&self, path: PathBuf, bytes: &[u8]) -> Result<()> {
        let directory = path
            .parent()
            .expect("cache entries always have an Account cache directory");
        fs::create_dir_all(directory).context(DirectorySnafu {
            path: directory.to_path_buf(),
        })?;
        let partial = path.with_extension("partial");
        fs::write(&partial, bytes).context(WriteSnafu {
            path: partial.clone(),
        })?;
        fs::rename(&partial, &path).context(WriteSnafu { path })
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

    fn all_entries(&self) -> Result<Vec<Entry>> {
        let mut entries = self.entries()?;
        for kind in [CacheKind::Media, CacheKind::Thumbnail] {
            collect_retained_entries(
                &self.root.join(kind.directory()).join("retained"),
                &mut entries,
            )?;
        }
        Ok(entries)
    }
}

fn collect_retained_entries(directory: &std::path::Path, entries: &mut Vec<Entry>) -> Result<()> {
    let children = match fs::read_dir(directory) {
        Ok(children) => children,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(Error::Directory {
                path: directory.to_path_buf(),
                source,
            });
        }
    };
    for child in children {
        let child = child.context(DirectorySnafu {
            path: directory.to_path_buf(),
        })?;
        let path = child.path();
        let metadata = child.metadata().context(ReadSnafu { path: path.clone() })?;
        if metadata.is_dir() {
            collect_retained_entries(&path, entries)?;
        } else if metadata.is_file() && path.extension().is_some_and(|value| value == "cache") {
            entries.push(Entry {
                path,
                bytes: metadata.len(),
                used: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }
    Ok(())
}

#[derive(Debug)]
struct Entry {
    path: PathBuf,
    bytes: u64,
    used: SystemTime,
}

#[cfg(test)]
mod tests;
