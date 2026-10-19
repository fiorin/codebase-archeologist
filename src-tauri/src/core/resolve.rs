use crate::core::types::Language;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Resolves import specifiers to indexed files.
///
/// Resolution order mirrors what TypeScript tooling does, in simplified form:
/// 1. Relative paths (`./`, `../`) with extension probing
/// 2. `tsconfig.json` / `jsconfig.json` `paths` aliases (plus `baseUrl`)
/// 3. Node-style bare specifiers → not resolved locally (external)
pub struct Resolver {
    /// alias prefix (e.g. "@app") -> replacement templates
    path_aliases: Vec<(String, Vec<String>)>,
    base_url: Option<String>,
    /// known relative paths, lowercased for windows-insensitive matching
    known: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct TsConfig {
    #[serde(default)]
    compiler_options: Option<TsCompilerOptions>,
}

#[derive(Debug, Deserialize)]
struct TsCompilerOptions {
    #[serde(default)]
    paths: Option<HashMap<String, Vec<String>>>,
    #[serde(default)]
    base_url: Option<String>,
}

impl Resolver {
    pub fn new(root: &Path) -> Self {
        let mut path_aliases = Vec::new();
        let mut base_url = None;

        for candidate in ["tsconfig.json", "jsconfig.json"] {
            let p = root.join(candidate);
            if let Ok(text) = std::fs::read_to_string(&p) {
                // tsconfig often contains comments; strip them crudely.
                let cleaned = strip_json_comments(&text);
                if let Ok(cfg) = serde_json::from_str::<TsConfig>(&cleaned) {
                    if let Some(opts) = cfg.compiler_options {
                        if let Some(bu) = opts.base_url {
                            base_url = Some(bu);
                        }
                        if let Some(paths) = opts.paths {
                            for (k, v) in paths {
                                path_aliases.push((k, v));
                            }
                        }
                    }
                }
                break;
            }
        }

        Resolver {
            path_aliases,
            base_url,
            known: HashMap::new(),
        }
    }

    pub fn build_index(&mut self, rel_paths: impl Iterator<Item = String>) {
        self.known.clear();
        for p in rel_paths {
            let key = normalize_key(&p);
            self.known.entry(key).or_insert(p);
        }
    }

    /// Returns (resolved_rel_path, method, confidence).
    pub fn resolve(
        &self,
        importer_rel: &str,
        specifier: &str,
    ) -> Option<(String, &'static str, f32)> {
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let base = Path::new(importer_rel)
                .parent()
                .unwrap_or_else(|| Path::new(""));
            let joined = normalize_slashes(&join_relative(base, specifier));
            return self.probe(&joined, "relative", 1.0);
        }

        for (prefix, templates) in &self.path_aliases {
            if let Some(rest) = strip_prefix_glob(prefix, specifier) {
                for template in templates {
                    let target = template.replace('*', rest);
                    let normalized = normalize_slashes(&target);
                    if let Some(found) = self.probe(&normalized, "tsconfig_paths", 0.95) {
                        return Some(found);
                    }
                }
            }
        }

        if let Some(base_url) = &self.base_url {
            let candidate =
                normalize_slashes(&format!("{}/{}", base_url.trim_end_matches('/'), specifier));
            if let Some(found) = self.probe(&candidate, "base_url", 0.85) {
                return Some(found);
            }
        }

        None
    }

    fn probe(
        &self,
        path: &str,
        method: &'static str,
        confidence: f32,
    ) -> Option<(String, &'static str, f32)> {
        let mut candidates: Vec<String> = Vec::new();
        candidates.push(path.to_string());
        for ext in [
            ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs", ".mts", ".cts", ".json",
        ] {
            candidates.push(format!("{path}{ext}"));
        }
        for index in ["index.ts", "index.tsx", "index.js", "index.jsx"] {
            candidates.push(format!("{path}/{index}"));
        }

        for c in &candidates {
            if let Some(found) = self.known.get(&normalize_key(c)) {
                return Some((found.clone(), method, confidence));
            }
        }
        None
    }

    pub fn is_external(specifier: &str) -> bool {
        !(specifier.starts_with("./")
            || specifier.starts_with("../")
            || specifier.starts_with('/')
            || Path::new(specifier).is_absolute())
    }
}

fn normalize_key(p: &str) -> String {
    p.replace('\\', "/").to_ascii_lowercase()
}

fn normalize_slashes(p: &str) -> String {
    p.replace('\\', "/")
}

fn join_relative(base: &Path, spec: &str) -> String {
    let mut parts: Vec<String> = base
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();

    for comp in Path::new(spec).components() {
        match comp {
            std::path::Component::ParentDir => {
                parts.pop();
            }
            std::path::Component::CurDir => {}
            std::path::Component::Normal(p) => parts.push(p.to_string_lossy().to_string()),
            other => parts.push(other.as_os_str().to_string_lossy().to_string()),
        }
    }
    parts.join("/")
}

fn strip_prefix_glob<'a>(prefix: &str, specifier: &'a str) -> Option<&'a str> {
    if let Some(star) = prefix.find('*') {
        let head = &prefix[..star];
        let tail = &prefix[star + 1..];
        if !specifier.starts_with(head) {
            return None;
        }
        let rest = &specifier[head.len()..];
        if !rest.ends_with(tail) {
            return None;
        }
        Some(&rest[..rest.len() - tail.len()])
    } else if specifier == prefix {
        Some("")
    } else {
        None
    }
}

pub fn language_of(rel: &str) -> Language {
    Language::from_path(rel)
}

fn strip_json_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_str = false;
    let mut prev = ' ';
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_str = false;
            }
            prev = c;
            continue;
        }
        match c {
            '"' => {
                in_str = true;
                out.push(c);
            }
            '/' if chars.peek() == Some(&'/') => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                let _ = chars.next();
                while let Some(n) = chars.next() {
                    if n == '*' && chars.peek() == Some(&'/') {
                        let _ = chars.next();
                        break;
                    }
                }
            }
            _ => out.push(c),
        }
        prev = c;
    }
    let _ = prev;
    out
}

pub fn resolve_tsconfig_paths_for_test(root: &Path) -> PathBuf {
    root.to_path_buf()
}
