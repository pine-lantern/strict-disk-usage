//! Recursive disk usage scanning, strict by default.
//!
//! A scan walks the direct children of a root directory and sums the
//! apparent size (`len()` of regular files) under each one. Any I/O
//! error hit along the way — permission denied, a broken symlink, a
//! directory that disappears mid-walk — aborts the whole scan unless
//! `ScanOptions.lenient` is set, in which case the offending entry is
//! skipped and recorded as a warning instead.
//!
//! The point of the strict default: a partial total that looks
//! complete is worse than no total at all. `du` on most systems will
//! print a size next to a stream of "permission denied" lines on
//! stderr, and it is easy for a script (or a human skimming a long
//! scrollback) to use that size without noticing the stream. Here you
//! either get a number you can trust, or an error, or you explicitly
//! asked for a best-effort number with `--lenient`.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Controls how a scan reacts to I/O errors encountered while walking,
/// and how much of the tree it breaks out into separate entries.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScanOptions {
    /// If false (the default), the first I/O error aborts the scan.
    /// If true, the offending entry is skipped and added to
    /// `ScanReport.warnings` instead.
    pub lenient: bool,
    /// How many levels below `root` get their own `Entry` in the
    /// report. `None` behaves like `Some(1)`: only `root`'s direct
    /// children are listed. This never affects `total_bytes`, which
    /// always reflects a full recursive scan regardless of depth.
    pub max_depth: Option<u32>,
}

/// One skipped entry, only ever populated in lenient mode.
#[derive(Debug)]
pub struct Warning {
    pub path: PathBuf,
    pub message: String,
}

/// A single entry in the report: a path and its total size, with any
/// deeper breakdown `ScanOptions.max_depth` allowed.
#[derive(Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub bytes: u64,
    /// Direct children of this entry, sorted largest first. Empty
    /// unless this entry is a directory and its depth was within
    /// `max_depth`.
    pub children: Vec<Entry>,
}

/// The result of a completed scan.
#[derive(Debug)]
pub struct ScanReport {
    pub root: PathBuf,
    pub total_bytes: u64,
    /// Direct children of `root`, sorted largest first.
    pub entries: Vec<Entry>,
    pub warnings: Vec<Warning>,
}

/// A fatal I/O error hit during a strict scan, tagged with the path
/// that caused it.
#[derive(Debug)]
pub struct ScanError {
    pub path: PathBuf,
    pub source: io::Error,
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.source)
    }
}

impl std::error::Error for ScanError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Scan `root` and report the size of each of its direct children.
///
/// `root` itself may be a plain file, in which case the report holds
/// a single entry for it.
pub fn scan(root: &Path, options: ScanOptions) -> Result<ScanReport, ScanError> {
    let mut report = ScanReport {
        root: root.to_path_buf(),
        total_bytes: 0,
        entries: Vec::new(),
        warnings: Vec::new(),
    };

    let root_meta = fs::symlink_metadata(root).map_err(|e| ScanError {
        path: root.to_path_buf(),
        source: e,
    })?;

    if !root_meta.is_dir() {
        report.total_bytes = root_meta.len();
        report.entries.push(Entry {
            path: root.to_path_buf(),
            bytes: root_meta.len(),
            children: Vec::new(),
        });
        return Ok(report);
    }

    let read_dir = fs::read_dir(root).map_err(|e| ScanError {
        path: root.to_path_buf(),
        source: e,
    })?;

    for item in read_dir {
        let dir_entry = match item {
            Ok(d) => d,
            Err(e) => {
                if options.lenient {
                    report.warnings.push(Warning {
                        path: root.to_path_buf(),
                        message: e.to_string(),
                    });
                    continue;
                }
                return Err(ScanError {
                    path: root.to_path_buf(),
                    source: e,
                });
            }
        };

        let path = dir_entry.path();
        match subtree_entry(&path, &options, 1, &mut report) {
            Ok(entry) => {
                report.total_bytes += entry.bytes;
                report.entries.push(entry);
            }
            Err(e) => {
                if options.lenient {
                    report.warnings.push(Warning {
                        path: e.path,
                        message: e.source.to_string(),
                    });
                } else {
                    return Err(e);
                }
            }
        }
    }

    report.entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    Ok(report)
}

/// Build the entry for everything under `path`, at the given depth
/// below `root` (`path`'s own direct children are `depth + 1`).
///
/// The returned `bytes` always reflects the full subtree regardless of
/// `max_depth` — only whether the breakdown is kept in `children` is
/// depth-limited. In lenient mode this never fails: unreadable
/// subtrees just contribute 0 and are logged to `report.warnings`.
fn subtree_entry(
    path: &Path,
    options: &ScanOptions,
    depth: u32,
    report: &mut ScanReport,
) -> Result<Entry, ScanError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => return recover(path, e, options, report),
    };

    if metadata.is_file() {
        return Ok(Entry {
            path: path.to_path_buf(),
            bytes: metadata.len(),
            children: Vec::new(),
        });
    }

    if !metadata.is_dir() {
        // A symlink, socket, fifo, or other special file. Its "size on
        // disk" is not a well-defined number of bytes of content, so
        // strict mode treats it as something the caller should look at
        // rather than silently folding into a total.
        let err = io::Error::new(
            io::ErrorKind::Other,
            "not a regular file or directory (symlink or special file)",
        );
        return recover(path, err, options, report);
    }

    let read_dir = match fs::read_dir(path) {
        Ok(rd) => rd,
        Err(e) => return recover(path, e, options, report),
    };

    let keep_children = depth < options.max_depth.unwrap_or(1);
    let mut total = 0u64;
    let mut children = Vec::new();
    for item in read_dir {
        match item {
            Ok(child) => match subtree_entry(&child.path(), options, depth + 1, report) {
                Ok(entry) => {
                    total += entry.bytes;
                    if keep_children {
                        children.push(entry);
                    }
                }
                Err(e) => {
                    if options.lenient {
                        report.warnings.push(Warning {
                            path: e.path,
                            message: e.source.to_string(),
                        });
                    } else {
                        return Err(e);
                    }
                }
            },
            Err(e) => return recover(path, e, options, report),
        }
    }

    children.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    Ok(Entry {
        path: path.to_path_buf(),
        bytes: total,
        children,
    })
}

/// Either bail out with a `ScanError` (strict) or record a warning and
/// carry on with a zero-byte, childless entry (lenient).
fn recover(
    path: &Path,
    err: io::Error,
    options: &ScanOptions,
    report: &mut ScanReport,
) -> Result<Entry, ScanError> {
    if options.lenient {
        report.warnings.push(Warning {
            path: path.to_path_buf(),
            message: err.to_string(),
        });
        Ok(Entry {
            path: path.to_path_buf(),
            bytes: 0,
            children: Vec::new(),
        })
    } else {
        Err(ScanError {
            path: path.to_path_buf(),
            source: err,
        })
    }
}
