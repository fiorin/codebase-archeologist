use crate::core::error::Result;
use crate::core::types::Language;
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ScannedFile {
    pub rel_path: String,
    pub abs_path: PathBuf,
    pub language: Language,
    pub size: u64,
    pub mtime: i64,
    pub hash: String,
    pub line_count: u32,
}

pub struct ScanOptions {
    pub follow_links: bool,
    pub max_file_size: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions {
            follow_links: false,
            max_file_size: 10 * 1024 * 1024,
        }
    }
}

const DEFAULT_EXCLUDES: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    "target",
    "coverage",
    ".next",
    ".cache",
    "vendor",
    "bin",
    "obj",
    ".archaeologist",
];

pub fn scan(
    root: &Path,
    opts: &ScanOptions,
    mut on_progress: impl FnMut(&str),
) -> Result<Vec<ScannedFile>> {
    let mut files = Vec::new();

    let mut builder = WalkBuilder::new(root);
    builder.follow_links(opts.follow_links);
    builder.hidden(true);
    builder.git_ignore(true);
    builder.git_global(true);
    builder.git_exclude(true);
    builder.parents(true);

    // Apply default directory exclusions via a filter entry.
    builder.filter_entry(|entry| {
        if entry.depth() == 0 {
            return true;
        }
        let name = entry.file_name().to_string_lossy();
        if DEFAULT_EXCLUDES.contains(&name.as_ref()) {
            return false;
        }
        true
    });

    for result in builder.build() {
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };

        if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }

        let path = entry.path();
        let lang = Language::from_path(&path.to_string_lossy());
        if !lang.is_target() {
            continue;
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.len() > opts.max_file_size {
            continue;
        }

        // Skip obvious generated files by name heuristic.
        let file_name = entry.file_name().to_string_lossy().to_string();
        if file_name.ends_with(".d.ts") && file_name.contains(".generated.") {
            continue;
        }

        on_progress(&path.to_string_lossy());

        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        // Skip binary-looking content.
        if looks_binary(&bytes) {
            continue;
        }

        let line_count = count_lines(&bytes);
        let hash = blake3::hash(&bytes).to_hex().to_string();
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let rel = pathdiff::diff_paths(path, root)
            .unwrap_or_else(|| path.to_path_buf())
            .to_string_lossy()
            .replace('\\', "/");

        files.push(ScannedFile {
            rel_path: rel,
            abs_path: path.to_path_buf(),
            language: lang,
            size: meta.len(),
            mtime,
            hash,
            line_count,
        });
    }

    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(files)
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

fn count_lines(bytes: &[u8]) -> u32 {
    if bytes.is_empty() {
        return 0;
    }
    let mut n = bytes.iter().filter(|&&b| b == b'\n').count() as u32;
    if bytes.last() != Some(&b'\n') {
        n += 1;
    }
    n
}

fn looks_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(8000)];
    sample.contains(&0)
}
