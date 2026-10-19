use crate::core::error::{CoreError, Result};
use crate::core::graph::{DepGraph, compute_metrics, cycle_findings, find_cycles};
use crate::core::lang::{LanguageAnalyzer, analyzer_for, typescript::TypeScriptAnalyzer};
use crate::core::resolve::Resolver;
use crate::core::scanner::{self, ScanOptions};
use crate::core::types::*;
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const ANALYZER_VERSION: &str = "2";

pub struct IndexJob {
    pub cancelled: Arc<AtomicBool>,
}

impl IndexJob {
    pub fn new() -> Self {
        IndexJob {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexResult {
    pub run_id: String,
    pub repository_id: String,
    pub mode: String,
    pub files_scanned: u32,
    pub files_parsed: u32,
    pub files_skipped: u32,
    pub symbols: u32,
    pub edges: u32,
    pub findings: u32,
    pub cycles: u32,
    pub duration_ms: u64,
}

pub struct Indexer<'a> {
    conn: &'a mut Connection,
    root: PathBuf,
    job: &'a IndexJob,
    emit: &'a mut dyn FnMut(ProgressEvent),
}

impl<'a> Indexer<'a> {
    pub fn new(
        conn: &'a mut Connection,
        root: PathBuf,
        job: &'a IndexJob,
        emit: &'a mut dyn FnMut(ProgressEvent),
    ) -> Self {
        Indexer {
            conn,
            root,
            job,
            emit,
        }
    }

    fn check_cancel(&self) -> Result<()> {
        if self.job.is_cancelled() {
            Err(CoreError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub fn ensure_repository(&mut self) -> Result<String> {
        let root_str = self.root.to_string_lossy().to_string();
        let existing: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM repositories WHERE root_path = ?1",
                params![root_str],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            return Ok(id);
        }
        let name = self
            .root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root_str.clone());
        let id = stable_id("repo", &root_str);
        self.conn.execute(
            "INSERT INTO repositories (id, root_path, name, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![id, root_str, name, now_iso()],
        )?;
        Ok(id)
    }

    pub fn index(&mut self, mode: &str) -> Result<IndexResult> {
        let repository_id = self.ensure_repository()?;
        let run_id = stable_id("run", &format!("{repository_id}-{}-{}", now_iso(), mode));
        self.conn.execute(
            "INSERT INTO analysis_runs (id, repository_id, started_at, status, mode, analyzer_version)
             VALUES (?1, ?2, ?3, 'running', ?4, ?5)",
            params![run_id, repository_id, now_iso(), mode, ANALYZER_VERSION],
        )?;

        let result = self.index_inner(&repository_id, &run_id, mode);

        match &result {
            Ok(r) => {
                self.conn.execute(
                    "UPDATE analysis_runs SET status='completed', finished_at=?1, files_scanned=?2,
                     files_parsed=?3, files_skipped=?4, symbols=?5, edges=?6, findings=?7 WHERE id=?8",
                    params![
                        now_iso(),
                        r.files_scanned,
                        r.files_parsed,
                        r.files_skipped,
                        r.symbols,
                        r.edges,
                        r.findings,
                        run_id
                    ],
                )?;
                self.conn.execute(
                    "UPDATE repositories SET last_indexed=?1 WHERE id=?2",
                    params![now_iso(), repository_id],
                )?;
            }
            Err(e) => {
                let cancelled = matches!(e, CoreError::Cancelled);
                self.conn.execute(
                    "UPDATE analysis_runs SET status=?1, finished_at=?2, error=?3 WHERE id=?4",
                    params![
                        if cancelled { "cancelled" } else { "failed" },
                        now_iso(),
                        e.to_string(),
                        run_id
                    ],
                )?;
            }
        }

        result
    }

    fn index_inner(
        &mut self,
        repository_id: &str,
        run_id: &str,
        mode: &str,
    ) -> Result<IndexResult> {
        let started = std::time::Instant::now();
        (self.emit)(ProgressEvent {
            kind: "index.started".into(),
            message: "Discovering files".into(),
            current: 0,
            total: 0,
        });

        let opts = ScanOptions::default();
        let scanned = scanner::scan(&self.root, &opts, |_| {})?;
        let total = scanned.len() as u64;
        self.check_cancel()?;

        // Existing index state
        let existing: HashMap<String, (String, String, i64)> = {
            let mut stmt = self.conn.prepare(
                "SELECT path, hash, analyzer_version, mtime FROM files WHERE repository_id = ?1",
            )?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            })?;
            let mut m = HashMap::new();
            for row in rows {
                let (path, hash, av, mtime) = row?;
                m.insert(path, (hash, av, mtime));
            }
            m
        };

        let scanned_paths: HashSet<String> = scanned.iter().map(|f| f.rel_path.clone()).collect();

        let mut files_parsed = 0u32;
        let mut files_skipped = 0u32;
        let mut symbols_total = 0u32;

        let mut analyzer_cache: HashMap<String, Box<dyn LanguageAnalyzer>> = HashMap::new();

        for (i, file) in scanned.iter().enumerate() {
            if i % 50 == 0 {
                self.check_cancel()?;
                (self.emit)(ProgressEvent {
                    kind: "index.progress".into(),
                    message: file.rel_path.clone(),
                    current: i as u64,
                    total,
                });
            }

            let unchanged = existing
                .get(&file.rel_path)
                .map(|(h, av, mt)| {
                    h == &file.hash && av == ANALYZER_VERSION && *mt == file.mtime as i64
                })
                .unwrap_or(false);

            if unchanged && mode == "incremental" {
                files_skipped += 1;
                continue;
            }

            let file_id = stable_id("file", &format!("{repository_id}|{}", file.rel_path));

            let content = std::fs::read_to_string(&file.abs_path).unwrap_or_default();
            let analyzer = analyzer_cache
                .entry(file.language.as_str().to_string())
                .or_insert_with(|| match analyzer_for(file.language) {
                    Some(a) => a,
                    None => Box::new(TypeScriptAnalyzer::new(Language::TypeScript)),
                });

            let extraction = analyzer.extract(&content);

            let tx = self.conn.transaction()?;
            tx.execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])?;
            tx.execute("DELETE FROM imports WHERE file_id = ?1", params![file_id])?;
            tx.execute("DELETE FROM calls WHERE file_id = ?1", params![file_id])?;
            tx.execute("DELETE FROM files WHERE id = ?1", params![file_id])?;

            tx.execute(
                "INSERT INTO files (id, repository_id, path, language, size, mtime, hash, line_count,
                 parse_status, parse_errors, analyzer_version, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    file_id,
                    repository_id,
                    file.rel_path,
                    file.language.as_str(),
                    file.size as i64,
                    file.mtime as i64,
                    file.hash,
                    file.line_count as i64,
                    if extraction.parse_errors > 0 { "partial" } else { "ok" },
                    extraction.parse_errors as i64,
                    ANALYZER_VERSION,
                    now_iso()
                ],
            )?;

            let mut symbol_id_map: HashMap<String, String> = HashMap::new();
            for s in &extraction.symbols {
                let sid = stable_id(
                    "sym",
                    &format!(
                        "{file_id}|{}|{}|{}",
                        s.qualified_name,
                        s.kind.as_str(),
                        s.start_line
                    ),
                );
                let parent_id = s
                    .parent_path
                    .as_ref()
                    .and_then(|p| symbol_id_map.get(p))
                    .cloned();
                tx.execute(
                    "INSERT OR IGNORE INTO symbols (id, file_id, parent_id, name, qualified_name, kind,
                     start_line, start_column, end_line, end_column, exported, signature, complexity)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        sid,
                        file_id,
                        parent_id,
                        s.name,
                        s.qualified_name,
                        s.kind.as_str(),
                        s.start_line as i64,
                        s.start_column as i64,
                        s.end_line as i64,
                        s.end_column as i64,
                        s.exported as i64,
                        s.signature,
                        s.complexity.map(i64::from)
                    ],
                )?;
                symbol_id_map.insert(s.qualified_name.clone(), sid);
            }

            for imp in &extraction.imports {
                tx.execute(
                    "INSERT INTO imports (file_id, specifier, kind, start_line, start_column, end_line,
                     end_column, resolved, resolved_file_id, resolution_method, confidence)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, NULL, NULL, 0.0)",
                    params![
                        file_id,
                        imp.specifier,
                        imp.kind,
                        imp.start_line as i64,
                        imp.start_column as i64,
                        imp.end_line as i64,
                        imp.end_column as i64
                    ],
                )?;
            }

            for call in &extraction.calls {
                tx.execute(
                    "INSERT INTO calls (file_id, caller_symbol, callee, start_line, confidence)
                     VALUES (?1, NULL, ?2, ?3, ?4)",
                    params![
                        file_id,
                        call.callee,
                        call.start_line as i64,
                        call.confidence
                    ],
                )?;
            }

            tx.commit()?;
            files_parsed += 1;
            symbols_total += extraction.symbols.len() as u32;

            (self.emit)(ProgressEvent {
                kind: "index.file.completed".into(),
                message: file.rel_path.clone(),
                current: (i + 1) as u64,
                total,
            });
        }

        // Remove stale files (deleted / renamed away)
        let stale: Vec<String> = existing
            .keys()
            .filter(|p| !scanned_paths.contains(*p))
            .cloned()
            .collect();
        for p in &stale {
            let fid = stable_id("file", &format!("{repository_id}|{p}"));
            self.conn
                .execute("DELETE FROM files WHERE id = ?1", params![fid])?;
        }
        self.conn
            .execute("DELETE FROM files WHERE repository_id = ?1 AND path NOT IN (SELECT value FROM json_each(?2))",
                params![repository_id, serde_json::to_string(&scanned_paths.iter().collect::<Vec<_>>())?])?;

        self.check_cancel()?;

        (self.emit)(ProgressEvent {
            kind: "index.progress".into(),
            message: "Resolving imports".into(),
            current: total,
            total,
        });

        let edges = self.resolve_imports(repository_id)?;
        let findings_count = self.run_analysis(repository_id, run_id)?;

        let edge_count: u32 = self.conn.query_row(
            "SELECT COUNT(*) FROM graph_edges WHERE repository_id = ?1 AND edge_type = 'imports'",
            params![repository_id],
            |r| r.get(0),
        )?;

        (self.emit)(ProgressEvent {
            kind: "index.completed".into(),
            message: "Index complete".into(),
            current: total,
            total,
        });

        let _ = edges;
        Ok(IndexResult {
            run_id: run_id.to_string(),
            repository_id: repository_id.to_string(),
            mode: mode.to_string(),
            files_scanned: scanned.len() as u32,
            files_parsed,
            files_skipped,
            symbols: symbols_total,
            edges: edge_count,
            findings: findings_count,
            cycles: self.count_cycles(repository_id),
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn resolve_imports(&mut self, repository_id: &str) -> Result<u32> {
        // File path -> id
        let mut path_to_id: HashMap<String, String> = HashMap::new();
        let mut all_paths: Vec<String> = Vec::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT id, path FROM files WHERE repository_id = ?1")?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (id, path) = row?;
                path_to_id.insert(path.clone(), id);
                all_paths.push(path);
            }
        }

        let mut resolver = Resolver::new(&self.root);
        resolver.build_index(all_paths.iter().cloned());

        // Load all imports
        let imports: Vec<(String, String, String, u32, String)> = {
            let mut stmt = self.conn.prepare(
                "SELECT i.file_id, i.specifier, f.path, i.id, i.kind
                 FROM imports i JOIN files f ON f.id = i.file_id
                 WHERE f.repository_id = ?1",
            )?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)? as u32,
                    r.get::<_, String>(4)?,
                ))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };

        let mut resolved_count = 0u32;
        let mut new_edges: Vec<GraphEdge> = Vec::new();

        let tx = self.conn.transaction()?;
        for (file_id, specifier, importer_path, import_row_id, kind) in &imports {
            let (target, method, confidence) = match resolver.resolve(importer_path, specifier) {
                Some(r) => r,
                None => {
                    if Resolver::is_external(specifier) {
                        tx.execute(
                            "UPDATE imports SET resolved=1, resolution_method='external', confidence=1.0 WHERE id=?1",
                            params![import_row_id],
                        )?;
                    }
                    continue;
                }
            };
            let Some(target_id) = path_to_id.get(&target) else {
                continue;
            };

            tx.execute(
                "UPDATE imports SET resolved=1, resolved_file_id=?1, resolution_method=?2, confidence=?3 WHERE id=?4",
                params![target_id, method, confidence, import_row_id],
            )?;
            resolved_count += 1;

            let edge_id = stable_id(
                "edge",
                &format!("{repository_id}|{file_id}|{target_id}|imports"),
            );
            tx.execute(
                "INSERT INTO graph_edges (id, repository_id, source_id, target_id, edge_type, confidence,
                 resolution_method, evidence_path, evidence_line, evidence_description)
                 VALUES (?1, ?2, ?3, ?4, 'imports', ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(repository_id, source_id, target_id, edge_type) DO UPDATE SET
                   confidence=excluded.confidence, resolution_method=excluded.resolution_method,
                   evidence_path=excluded.evidence_path, evidence_line=excluded.evidence_line,
                   evidence_description=excluded.evidence_description",
                params![
                    edge_id,
                    repository_id,
                    file_id,
                    target_id,
                    confidence,
                    method,
                    importer_path,
                    0i64,
                    format!("{kind} `{specifier}` → {target}")
                ],
            )?;

            new_edges.push(GraphEdge {
                id: edge_id,
                source: file_id.clone(),
                target: target_id.clone(),
                edge_type: "imports".into(),
                confidence,
                resolution_method: Some(method.to_string()),
                evidence: Evidence::source(
                    importer_path.clone(),
                    file_id.clone(),
                    0,
                    0,
                    format!("{kind} `{specifier}` resolves to {target}"),
                ),
            });
        }

        // Remove edges whose source file no longer exists or specifier gone
        tx.execute(
            "DELETE FROM graph_edges WHERE repository_id=?1 AND edge_type='imports' AND source_id NOT IN (SELECT id FROM files WHERE repository_id=?1)",
            params![repository_id],
        )?;
        tx.commit()?;

        let _ = new_edges;
        Ok(resolved_count)
    }

    fn run_analysis(&mut self, repository_id: &str, run_id: &str) -> Result<u32> {
        (self.emit)(ProgressEvent {
            kind: "analysis.started".into(),
            message: "Computing graph analyses".into(),
            current: 0,
            total: 0,
        });

        // Edges as GraphEdge
        let mut edges: Vec<GraphEdge> = Vec::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT id, source_id, target_id, edge_type, confidence, resolution_method,
                        evidence_path, evidence_line, evidence_description
                 FROM graph_edges WHERE repository_id = ?1",
            )?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok(GraphEdge {
                    id: r.get(0)?,
                    source: r.get(1)?,
                    target: r.get(2)?,
                    edge_type: r.get(3)?,
                    confidence: r.get(4)?,
                    resolution_method: r.get(5)?,
                    evidence: Evidence {
                        kind: "source".into(),
                        file_id: None,
                        path: r.get(6)?,
                        start_line: r.get::<_, Option<i64>>(7)?.map(|v| v as u32),
                        end_line: None,
                        description: r.get(8)?,
                    },
                })
            })?;
            for row in rows {
                edges.push(row?);
            }
        }

        // file_id -> path
        let mut paths: HashMap<String, String> = HashMap::new();
        let mut complexities: HashMap<String, u32> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT id, path FROM files WHERE repository_id = ?1")?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (id, path) = row?;
                paths.insert(id, path);
            }
        }
        {
            let mut stmt = self.conn.prepare(
                "SELECT file_id, MAX(COALESCE(complexity,0)) FROM symbols WHERE file_id IN
                 (SELECT id FROM files WHERE repository_id = ?1) GROUP BY file_id",
            )?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)? as u32))
            })?;
            for row in rows {
                let (id, c) = row?;
                complexities.insert(id, c);
            }
        }

        let label = |id: &str| paths.get(id).cloned().unwrap_or_else(|| id.to_string());

        let g = DepGraph::build(&edges);
        let cycles = find_cycles(&g, label);
        let mut findings = cycle_findings(&cycles, label);
        findings.extend(self.static_findings(repository_id, &paths, &g)?);
        let churn = git_churn(&self.root, &paths);
        let (metrics, hotspots) = compute_metrics(&g, &complexities, &churn, &paths);

        // Persist metrics
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM metrics WHERE repository_id = ?1",
            params![repository_id],
        )?;
        for m in &metrics {
            tx.execute(
                "INSERT INTO metrics (repository_id, file_id, fan_in, fan_out, instability, complexity,
                 change_frequency, hotspot_score) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    repository_id,
                    m.file_id,
                    m.fan_in as i64,
                    m.fan_out as i64,
                    m.instability,
                    m.complexity as i64,
                    m.change_frequency as i64,
                    m.hotspot_score
                ],
            )?;
        }

        tx.execute(
            "DELETE FROM findings WHERE repository_id = ?1",
            params![repository_id],
        )?;
        for f in &findings {
            tx.execute(
                "INSERT INTO findings (id, repository_id, analysis_run_id, severity, category, title,
                 description, confidence, evidence, affected_nodes, suggestions)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
                 ON CONFLICT(id) DO UPDATE SET description=excluded.description, evidence=excluded.evidence",
                params![
                    f.id,
                    repository_id,
                    run_id,
                    f.severity,
                    f.category,
                    f.title,
                    f.description,
                    f.confidence,
                    serde_json::to_string(&f.evidence)?,
                    serde_json::to_string(&f.affected_node_ids)?,
                    serde_json::to_string(&f.suggestions)?
                ],
            )?;
        }

        // Rebuild FTS search index
        tx.execute(
            "DELETE FROM search_index WHERE repository_id = ?1",
            params![repository_id],
        )?;
        {
            let mut stmt = tx.prepare(
                "SELECT f.id, f.path, s.name, s.qualified_name
                 FROM files f LEFT JOIN symbols s ON s.file_id = f.id
                 WHERE f.repository_id = ?1",
            )?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<String>>(3)?,
                ))
            })?;
            for row in rows {
                let (fid, path, sname, sq) = row?;
                tx.execute(
                    "INSERT INTO search_index (file_id, repository_id, path, symbol_name, qualified_name)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        fid,
                        repository_id,
                        path,
                        sname.unwrap_or_default(),
                        sq.unwrap_or_default()
                    ],
                )?;
            }
        }

        tx.commit()?;

        let _ = hotspots;
        (self.emit)(ProgressEvent {
            kind: "analysis.completed".into(),
            message: format!("{} findings", findings.len()),
            current: 0,
            total: 0,
        });

        Ok(findings.len() as u32)
    }

    fn static_findings(
        &self,
        repository_id: &str,
        paths: &HashMap<String, String>,
        graph: &DepGraph,
    ) -> Result<Vec<Finding>> {
        const COMPLEXITY_THRESHOLD: u32 = 15;
        const HIGH_FAN_OUT: u32 = 10;
        const HIGH_FAN_IN: u32 = 20;

        let mut findings = Vec::new();

        // A high per-symbol cyclomatic complexity is a maintainability signal,
        // not a claim that the code is defective.
        {
            let mut stmt = self.conn.prepare(
                "SELECT s.id, s.file_id, f.path, s.qualified_name, s.start_line, s.complexity
                 FROM symbols s JOIN files f ON f.id = s.file_id
                 WHERE f.repository_id = ?1 AND s.complexity >= ?2",
            )?;
            let rows = stmt.query_map(params![repository_id, COMPLEXITY_THRESHOLD], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)? as u32,
                    r.get::<_, i64>(5)? as u32,
                ))
            })?;
            for row in rows {
                let (symbol_id, file_id, path, name, line, complexity) = row?;
                findings.push(Finding {
                    id: stable_id("finding_complexity", &symbol_id),
                    severity: if complexity >= 25 { "high" } else { "medium" }.into(),
                    category: "complexity".into(),
                    title: format!("High complexity: {name}"),
                    description: format!(
                        "`{name}` has estimated cyclomatic complexity {complexity} (threshold {COMPLEXITY_THRESHOLD}). The estimate counts common branches and boolean conditions; it is a maintainability signal, not a defect verdict."
                    ),
                    evidence: vec![Evidence::source(
                        path,
                        file_id.clone(),
                        line,
                        line,
                        format!("Estimated cyclomatic complexity: {complexity}"),
                    )],
                    affected_node_ids: vec![file_id],
                    confidence: 0.85,
                    suggestions: vec![
                        "Consider extracting cohesive helpers or simplifying branching.".into(),
                    ],
                });
            }
        }

        // Report unresolved relative imports only; bare specifiers are usually
        // packages and are intentionally outside the local file graph.
        {
            let mut stmt = self.conn.prepare(
                "SELECT i.id, i.file_id, f.path, i.specifier, i.start_line
                 FROM imports i JOIN files f ON f.id = i.file_id
                 WHERE f.repository_id = ?1 AND i.resolved = 0
                   AND (i.specifier LIKE './%' OR i.specifier LIKE '../%' OR i.specifier LIKE '/%')",
            )?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)? as u32,
                ))
            })?;
            for row in rows {
                let (import_id, file_id, path, specifier, line) = row?;
                let key = format!("{file_id}|{specifier}|{line}|{import_id}");
                findings.push(Finding {
                    id: stable_id("finding_unresolved", &key),
                    severity: "medium".into(),
                    category: "unresolved_import".into(),
                    title: format!("Unresolved local import: {specifier}"),
                    description: format!(
                        "The local import `{specifier}` from `{path}` did not resolve to an indexed file. This can indicate a typo, a missing file, or an unsupported resolution rule."
                    ),
                    evidence: vec![Evidence::source(
                        path,
                        file_id.clone(),
                        line,
                        line,
                        format!("Unresolved import specifier `{specifier}`"),
                    )],
                    affected_node_ids: vec![file_id],
                    confidence: 0.95,
                    suggestions: vec![
                        "Check the import path, file extension/index convention, and tsconfig/jsconfig aliases.".into(),
                    ],
                });
            }
        }

        // File-level coupling findings use explicit, conservative thresholds.
        for file_id in paths.keys() {
            let fan_in = graph.fan_in(file_id);
            let fan_out = graph.fan_out(file_id);
            if fan_in < HIGH_FAN_IN && fan_out < HIGH_FAN_OUT {
                continue;
            }
            let file_id = file_id.as_str();
            let path = paths.get(file_id).cloned().unwrap_or_default();
            findings.push(Finding {
                id: stable_id("finding_coupling", file_id),
                severity: if fan_in >= HIGH_FAN_IN * 2 || fan_out >= HIGH_FAN_OUT * 2 {
                    "high"
                } else {
                    "medium"
                }
                .into(),
                category: "coupling".into(),
                title: format!("Highly coupled file: {path}"),
                description: format!(
                    "`{path}` has fan-in {fan_in} and fan-out {fan_out}. High fan-in increases the potential impact of changes; high fan-out can indicate broad dependency responsibilities. Thresholds: fan-in {HIGH_FAN_IN}, fan-out {HIGH_FAN_OUT}."
                ),
                evidence: vec![Evidence {
                    kind: "metric".into(),
                    file_id: Some(file_id.to_string()),
                    path: Some(path.clone()),
                    start_line: None,
                    end_line: None,
                    description: format!("fan-in={fan_in}, fan-out={fan_out}"),
                }],
                affected_node_ids: vec![file_id.to_string()],
                confidence: 0.9,
                suggestions: vec![
                    "Review whether this file has multiple responsibilities or whether dependents can target smaller interfaces.".into(),
                ],
            });
        }

        // Parser recovery is tolerant, so surface files whose syntax tree was
        // only partially recovered instead of silently treating it as clean.
        {
            let mut stmt = self.conn.prepare(
                "SELECT id, path, parse_errors FROM files
                 WHERE repository_id = ?1 AND parse_errors > 0",
            )?;
            let rows = stmt.query_map(params![repository_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)? as u32,
                ))
            })?;
            for row in rows {
                let (file_id, path, parse_errors) = row?;
                findings.push(Finding {
                    id: stable_id("finding_parse", &file_id),
                    severity: if parse_errors >= 10 { "high" } else { "medium" }.into(),
                    category: "parse_error".into(),
                    title: format!("Syntax parse issues: {path}"),
                    description: format!(
                        "Tree-sitter recovered from {parse_errors} syntax error or missing-node markers in `{path}`. Extracted symbols and dependencies from this file may be incomplete."
                    ),
                    evidence: vec![Evidence {
                        kind: "parse_diagnostic".into(),
                        file_id: Some(file_id.clone()),
                        path: Some(path),
                        start_line: None,
                        end_line: None,
                        description: format!("{parse_errors} parser diagnostics"),
                    }],
                    affected_node_ids: vec![file_id],
                    confidence: 0.95,
                    suggestions: vec![
                        "Check the source syntax and confirm the file uses syntax supported by the configured parser.".into(),
                    ],
                });
            }
        }

        Ok(findings)
    }

    fn count_cycles(&self, repository_id: &str) -> u32 {
        self.conn
            .query_row(
                "SELECT COUNT(*) FROM findings WHERE repository_id = ?1 AND category='cycle'",
                params![repository_id],
                |r| r.get(0),
            )
            .unwrap_or(0)
    }
}

/// Count per-file commits during the previous 90 days. If Git is unavailable,
/// the directory is not a repository, or the log cannot be read, churn remains
/// zero and the rest of the analysis still completes normally.
fn git_churn(root: &Path, paths: &HashMap<String, String>) -> HashMap<String, u32> {
    let prefix = Command::new("git")
        .args(["-C", &root.to_string_lossy(), "rev-parse", "--show-prefix"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| normalize_repo_path(String::from_utf8_lossy(&output.stdout).trim()))
        .unwrap_or_default();
    let output = Command::new("git")
        .args([
            "-C",
            &root.to_string_lossy(),
            "log",
            "--since=90.days",
            "--format=",
            "--name-only",
            "--no-renames",
        ])
        .output();
    let Ok(output) = output else {
        return HashMap::new();
    };
    if !output.status.success() {
        return HashMap::new();
    }

    let known: HashMap<String, String> = paths
        .iter()
        .map(|(file_id, path)| (normalize_repo_path(path), file_id.clone()))
        .collect();
    let mut counts = HashMap::new();
    for path in String::from_utf8_lossy(&output.stdout).lines() {
        let repo_path = normalize_repo_path(path.trim());
        let normalized = if prefix.is_empty() {
            repo_path
        } else {
            let Some(relative) = repo_path.strip_prefix(&prefix) else {
                continue;
            };
            relative.to_string()
        };
        if let Some(file_id) = known.get(&normalized) {
            *counts.entry(file_id.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn normalize_repo_path(path: &str) -> String {
    path.trim_start_matches("./").replace('\\', "/")
}

pub fn stable_id(prefix: &str, input: &str) -> String {
    let h = blake3::hash(input.as_bytes());
    let hex = h.to_hex();
    format!("{prefix}_{}", &hex[..24])
}

pub fn now_iso() -> String {
    // Avoid extra dependency: RFC3339-ish from SystemTime.
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{d}")
}

pub fn repo_root_of(path: &Path) -> Result<PathBuf> {
    if !path.is_dir() {
        return Err(CoreError::InvalidRepository(format!(
            "{} is not a directory",
            path.display()
        )));
    }
    Ok(path.to_path_buf())
}
