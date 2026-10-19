use crate::core::error::Result;
use crate::core::graph::DepGraph;
use crate::core::indexer::stable_id;
use crate::core::types::*;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize)]
pub struct FileDetail {
    pub file: FileRecord,
    pub symbols: Vec<SymbolRecord>,
    pub imports: Vec<ImportSummary>,
    pub dependents: Vec<ImportSummary>,
    pub metrics: Option<MetricRecord>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    pub path: String,
    pub file_id: String,
    pub specifier: String,
    pub kind: String,
    pub line: u32,
    pub confidence: f32,
    pub resolved: bool,
    pub resolution_method: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub file_count: u32,
    pub symbol_count: u32,
    pub module: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleEdge {
    pub source: String,
    pub target: String,
    pub weight: u32,
    pub edge_type: String,
    pub confidence: f32,
    pub evidence: Vec<Evidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphView {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub truncated: bool,
    pub level: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub file_id: String,
    pub path: String,
    pub symbol_name: Option<String>,
    pub qualified_name: Option<String>,
    pub kind: Option<String>,
    pub line: Option<u32>,
}

pub fn summary(conn: &Connection, repository_id: &str) -> Result<RepoSummary> {
    let (root, name, last_indexed): (String, String, Option<String>) = conn.query_row(
        "SELECT root_path, name, last_indexed FROM repositories WHERE id = ?1",
        params![repository_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;

    let file_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE repository_id = ?1",
        params![repository_id],
        |r| r.get(0),
    )?;
    let symbol_count: u32 = conn.query_row("SELECT COUNT(*) FROM symbols WHERE file_id IN (SELECT id FROM files WHERE repository_id = ?1)", params![repository_id], |r| r.get(0))?;
    let edge_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM graph_edges WHERE repository_id = ?1",
        params![repository_id],
        |r| r.get(0),
    )?;
    let finding_count: u32 = conn.query_row(
        "SELECT COUNT(*) FROM findings WHERE repository_id = ?1",
        params![repository_id],
        |r| r.get(0),
    )?;

    let mut languages = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT language, COUNT(*) FROM files WHERE repository_id = ?1 GROUP BY language ORDER BY COUNT(*) DESC",
        )?;
        let rows = stmt.query_map(params![repository_id], |r| {
            Ok(LanguageCount {
                language: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        for row in rows {
            languages.push(row?);
        }
    }

    Ok(RepoSummary {
        repository_id: repository_id.to_string(),
        root,
        name,
        file_count,
        symbol_count,
        edge_count,
        finding_count,
        languages,
        last_indexed,
    })
}

pub fn list_repositories(conn: &Connection) -> Result<Vec<RepoSummary>> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare("SELECT id FROM repositories ORDER BY created_at")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    ids.iter().map(|id| summary(conn, id)).collect()
}

pub fn list_files(
    conn: &Connection,
    repository_id: &str,
    limit: u32,
    offset: u32,
) -> Result<Vec<FileRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, repository_id, path, language, size, hash, line_count, parse_status, parse_errors
         FROM files WHERE repository_id = ?1 ORDER BY path LIMIT ?2 OFFSET ?3",
    )?;
    let rows = stmt.query_map(params![repository_id, limit, offset], |r| {
        Ok(FileRecord {
            id: r.get(0)?,
            repository_id: r.get(1)?,
            path: r.get(2)?,
            language: r.get(3)?,
            size: r.get::<_, i64>(4)? as u64,
            hash: r.get(5)?,
            line_count: r.get::<_, i64>(6)? as u32,
            parse_status: r.get(7)?,
            parse_errors: r.get::<_, i64>(8)? as u32,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn find_file(conn: &Connection, repository_id: &str, path: &str) -> Result<Option<FileRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, repository_id, path, language, size, hash, line_count, parse_status, parse_errors
         FROM files WHERE repository_id = ?1 AND path = ?2",
    )?;
    let rec = stmt
        .query_row(params![repository_id, path], |r| {
            Ok(FileRecord {
                id: r.get(0)?,
                repository_id: r.get(1)?,
                path: r.get(2)?,
                language: r.get(3)?,
                size: r.get::<_, i64>(4)? as u64,
                hash: r.get(5)?,
                line_count: r.get::<_, i64>(6)? as u32,
                parse_status: r.get(7)?,
                parse_errors: r.get::<_, i64>(8)? as u32,
            })
        })
        .optional()?;
    Ok(rec)
}

pub fn file_detail(conn: &Connection, repository_id: &str, file_id: &str) -> Result<FileDetail> {
    let file: FileRecord = conn.query_row(
        "SELECT id, repository_id, path, language, size, hash, line_count, parse_status, parse_errors
         FROM files WHERE id = ?1 AND repository_id = ?2",
        params![file_id, repository_id],
        |r| {
            Ok(FileRecord {
                id: r.get(0)?,
                repository_id: r.get(1)?,
                path: r.get(2)?,
                language: r.get(3)?,
                size: r.get::<_, i64>(4)? as u64,
                hash: r.get(5)?,
                line_count: r.get::<_, i64>(6)? as u32,
                parse_status: r.get(7)?,
                parse_errors: r.get::<_, i64>(8)? as u32,
            })
        },
    )?;

    let symbols: Vec<SymbolRecord> = {
        let mut stmt = conn.prepare(
            "SELECT id, file_id, parent_id, name, qualified_name, kind, start_line, start_column,
                    end_line, end_column, exported, signature, complexity
             FROM symbols WHERE file_id = ?1 ORDER BY start_line",
        )?;
        let rows = stmt.query_map(params![file_id], |r| {
            Ok(SymbolRecord {
                id: r.get(0)?,
                file_id: r.get(1)?,
                parent_id: r.get(2)?,
                name: r.get(3)?,
                qualified_name: r.get(4)?,
                kind: r.get(5)?,
                start_line: r.get::<_, i64>(6)? as u32,
                start_column: r.get::<_, i64>(7)? as u32,
                end_line: r.get::<_, i64>(8)? as u32,
                end_column: r.get::<_, i64>(9)? as u32,
                exported: r.get::<_, i64>(10)? != 0,
                signature: r.get(11)?,
                complexity: r.get::<_, Option<i64>>(12)?.map(|v| v as u32),
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let imports = import_summaries(conn, file_id, false)?;
    let dependents = import_summaries(conn, file_id, true)?;

    let metrics = conn
        .query_row(
            "SELECT file_id, fan_in, fan_out, instability, complexity, change_frequency, hotspot_score
             FROM metrics WHERE repository_id = ?1 AND file_id = ?2",
            params![repository_id, file_id],
            |r| {
                Ok(MetricRecord {
                    file_id: r.get(0)?,
                    path: String::new(),
                    fan_in: r.get::<_, i64>(1)? as u32,
                    fan_out: r.get::<_, i64>(2)? as u32,
                    instability: r.get(3)?,
                    complexity: r.get::<_, i64>(4)? as u32,
                    change_frequency: r.get::<_, i64>(5)? as u32,
                    hotspot_score: r.get(6)?,
                })
            },
        )
        .optional()?;

    let mut findings = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, severity, category, title, description, confidence, evidence, affected_nodes, suggestions
             FROM findings WHERE repository_id = ?1 AND affected_nodes LIKE '%' || ?2 || '%'",
        )?;
        let rows = stmt.query_map(params![repository_id, file_id], |r| {
            Ok(Finding {
                id: r.get(0)?,
                severity: r.get(1)?,
                category: r.get(2)?,
                title: r.get(3)?,
                description: r.get(4)?,
                confidence: r.get(5)?,
                evidence: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
                affected_node_ids: serde_json::from_str(&r.get::<_, String>(7)?)
                    .unwrap_or_default(),
                suggestions: serde_json::from_str(&r.get::<_, String>(8)?).unwrap_or_default(),
            })
        })?;
        for row in rows {
            findings.push(row?);
        }
    }

    Ok(FileDetail {
        file,
        symbols,
        imports,
        dependents,
        metrics,
        findings,
    })
}

fn import_summaries(conn: &Connection, file_id: &str, reverse: bool) -> Result<Vec<ImportSummary>> {
    let sql = if reverse {
        "SELECT f.path, i.file_id, i.specifier, i.kind, i.start_line, i.confidence, i.resolved, i.resolution_method
         FROM imports i JOIN files f ON f.id = i.file_id
         WHERE i.resolved_file_id = ?1 ORDER BY f.path"
    } else {
        "SELECT f.path, i.file_id, i.specifier, i.kind, i.start_line, i.confidence, i.resolved, i.resolution_method
         FROM imports i JOIN files f ON f.id = i.resolved_file_id
         WHERE i.file_id = ?1 ORDER BY f.path"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![file_id], |r| {
        Ok(ImportSummary {
            path: r.get(0)?,
            file_id: r.get(1)?,
            specifier: r.get(2)?,
            kind: r.get(3)?,
            line: r.get::<_, i64>(4)? as u32,
            confidence: r.get(5)?,
            resolved: r.get::<_, i64>(6)? != 0,
            resolution_method: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Builds a graph view. `level` is "file" or "module".
/// `focus` optionally limits to a single module prefix (directory).
pub fn graph_view(
    conn: &Connection,
    repository_id: &str,
    level: &str,
    focus: Option<&str>,
    max_nodes: usize,
) -> Result<GraphView> {
    let mut path_to_id: HashMap<String, String> = HashMap::new();
    let mut id_to_path: HashMap<String, String> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT id, path FROM files WHERE repository_id = ?1")?;
        let rows = stmt.query_map(params![repository_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, path) = row?;
            path_to_id.insert(path.clone(), id.clone());
            id_to_path.insert(id, path);
        }
    }

    let mut edges_raw: Vec<(
        String,
        String,
        String,
        f32,
        Option<String>,
        Option<String>,
        String,
    )> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, edge_type, confidence, resolution_method, evidence_path, evidence_description
             FROM graph_edges WHERE repository_id = ?1 AND edge_type = 'imports'",
        )?;
        let rows = stmt.query_map(params![repository_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)? as f32,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, String>(6)?,
            ))
        })?;
        for row in rows {
            edges_raw.push(row?);
        }
    }

    let module_of = |path: &str| -> String {
        match path.rfind('/') {
            Some(i) => path[..i].to_string(),
            None => "(root)".to_string(),
        }
    };

    let keep = |path: &str| -> bool {
        match focus {
            Some(f) if !f.is_empty() && f != "(root)" => path.starts_with(f),
            _ => true,
        }
    };

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut truncated = false;

    if level == "module" {
        // Aggregate file edges into module edges.
        let mut agg: HashMap<(String, String), (u32, f64, Vec<Evidence>)> = HashMap::new();
        for (src, tgt, _et, conf, _method, epath, edesc) in &edges_raw {
            let (Some(sp), Some(tp)) = (id_to_path.get(src), id_to_path.get(tgt)) else {
                continue;
            };
            if !keep(sp) || !keep(tp) {
                continue;
            }
            let sm = module_of(sp);
            let tm = module_of(tp);
            if sm == tm {
                continue;
            }
            let entry = agg
                .entry((sm.clone(), tm.clone()))
                .or_insert((0, 0.0, Vec::new()));
            entry.0 += 1;
            entry.1 += *conf as f64;
            if entry.2.len() < 3 {
                entry.2.push(Evidence::source(
                    epath.clone().unwrap_or_default(),
                    src.clone(),
                    0,
                    0,
                    edesc.clone(),
                ));
            }
        }

        let mut module_ids: HashSet<String> = HashSet::new();
        for (s, t) in agg.keys() {
            module_ids.insert(s.clone());
            module_ids.insert(t.clone());
        }
        let mut module_files: HashMap<String, u32> = HashMap::new();
        for path in id_to_path.values() {
            *module_files.entry(module_of(path)).or_insert(0) += 1;
        }

        let mut mods: Vec<String> = module_ids.into_iter().collect();
        mods.sort();
        if mods.len() > max_nodes {
            truncated = true;
            mods.truncate(max_nodes);
        }

        for m in mods {
            nodes.push(GraphNode {
                id: format!("mod:{}", m),
                kind: "module".into(),
                label: m.clone(),
                file_id: None,
                module: Some(m.clone()),
                data: serde_json::json!({ "fileCount": module_files.get(&m).copied().unwrap_or(0) }),
            });
        }

        for ((s, t), (w, conf, ev)) in agg {
            let sid = format!("mod:{s}");
            let tid = format!("mod:{t}");
            if !nodes.iter().any(|n| n.id == sid) || !nodes.iter().any(|n| n.id == tid) {
                continue;
            }
            edges.push(GraphEdge {
                id: stable_id("e", &format!("{sid}|{t}|module")),
                source: sid,
                target: tid,
                edge_type: "module_imports".into(),
                confidence: (conf / w as f64) as f32,
                resolution_method: Some("aggregated".into()),
                evidence: Evidence {
                    kind: "graph".into(),
                    file_id: None,
                    path: None,
                    start_line: None,
                    end_line: None,
                    description: format!("{w} file-level imports aggregated"),
                },
            });
            let _ = ev;
        }
    } else {
        // File level, filtered by focus.
        let mut visible: HashSet<String> = HashSet::new();
        let mut candidates: Vec<String> = id_to_path
            .iter()
            .filter(|(_, p)| keep(p))
            .map(|(id, _)| id.clone())
            .collect();
        candidates.sort();
        if candidates.len() > max_nodes {
            truncated = true;
            candidates.truncate(max_nodes);
        }
        for id in &candidates {
            visible.insert(id.clone());
        }

        for id in &candidates {
            let path = &id_to_path[id];
            nodes.push(GraphNode {
                id: id.clone(),
                kind: "file".into(),
                label: path.clone(),
                file_id: Some(id.clone()),
                module: Some(module_of(path)),
                data: serde_json::json!({ "path": path }),
            });
        }

        for (src, tgt, et, conf, method, epath, edesc) in &edges_raw {
            if !visible.contains(src) || !visible.contains(tgt) {
                continue;
            }
            edges.push(GraphEdge {
                id: stable_id("e", &format!("{src}|{tgt}|{et}")),
                source: src.clone(),
                target: tgt.clone(),
                edge_type: et.clone(),
                confidence: *conf,
                resolution_method: method.clone(),
                evidence: Evidence::source(
                    epath.clone().unwrap_or_default(),
                    src.clone(),
                    0,
                    0,
                    edesc.clone(),
                ),
            });
            if edges.len() > max_nodes * 3 {
                truncated = true;
                break;
            }
        }
    }

    Ok(GraphView {
        nodes,
        edges,
        truncated,
        level: level.to_string(),
    })
}

pub fn search(
    conn: &Connection,
    repository_id: &str,
    query: &str,
    limit: u32,
) -> Result<Vec<SearchResult>> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(vec![]);
    }

    // Support `symbol:` `file:` prefixes in a very simple way first.
    let (field, term) = if let Some(rest) = q.strip_prefix("symbol:") {
        ("symbol", rest.trim())
    } else if let Some(rest) = q.strip_prefix("file:") {
        ("file", rest.trim())
    } else {
        ("any", q)
    };

    let like = format!("%{}%", term.replace('%', "\\%").replace('_', "\\_"));

    let mut results = Vec::new();
    match field {
        "symbol" => {
            let mut stmt = conn.prepare(
                "SELECT s.file_id, f.path, s.name, s.qualified_name, s.kind, s.start_line
                 FROM symbols s JOIN files f ON f.id = s.file_id
                 WHERE f.repository_id = ?1 AND (s.name LIKE ?2 OR s.qualified_name LIKE ?2)
                 ORDER BY s.name LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![repository_id, like, limit], |r| {
                Ok(SearchResult {
                    file_id: r.get(0)?,
                    path: r.get(1)?,
                    symbol_name: r.get(2)?,
                    qualified_name: r.get(3)?,
                    kind: r.get(4)?,
                    line: Some(r.get::<_, i64>(5)? as u32),
                })
            })?;
            results = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        }
        "file" => {
            let mut stmt = conn.prepare(
                "SELECT id, path FROM files WHERE repository_id = ?1 AND path LIKE ?2 ORDER BY path LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![repository_id, like, limit], |r| {
                Ok(SearchResult {
                    file_id: r.get(0)?,
                    path: r.get(1)?,
                    symbol_name: None,
                    qualified_name: None,
                    kind: None,
                    line: None,
                })
            })?;
            results = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        }
        _ => {
            let mut stmt = conn.prepare(
                "SELECT s.file_id, f.path, s.name, s.qualified_name, s.kind, s.start_line
                 FROM symbols s JOIN files f ON f.id = s.file_id
                 WHERE f.repository_id = ?1 AND (s.name LIKE ?2 OR s.qualified_name LIKE ?2)
                 ORDER BY s.name LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![repository_id, like, limit], |r| {
                Ok(SearchResult {
                    file_id: r.get(0)?,
                    path: r.get(1)?,
                    symbol_name: r.get(2)?,
                    qualified_name: r.get(3)?,
                    kind: r.get(4)?,
                    line: Some(r.get::<_, i64>(5)? as u32),
                })
            })?;
            results.extend(rows.collect::<std::result::Result<Vec<_>, _>>()?);

            let mut stmt = conn.prepare(
                "SELECT id, path FROM files WHERE repository_id = ?1 AND path LIKE ?2 ORDER BY path LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![repository_id, like, limit], |r| {
                Ok(SearchResult {
                    file_id: r.get(0)?,
                    path: r.get(1)?,
                    symbol_name: None,
                    qualified_name: None,
                    kind: None,
                    line: None,
                })
            })?;
            results.extend(rows.collect::<std::result::Result<Vec<_>, _>>()?);
        }
    }

    Ok(results)
}

pub fn findings(conn: &Connection, repository_id: &str) -> Result<Vec<Finding>> {
    let mut stmt = conn.prepare(
        "SELECT id, severity, category, title, description, confidence, evidence, affected_nodes, suggestions
         FROM findings WHERE repository_id = ?1 ORDER BY
         CASE severity WHEN 'critical' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 ELSE 4 END",
    )?;
    let rows = stmt.query_map(params![repository_id], |r| {
        Ok(Finding {
            id: r.get(0)?,
            severity: r.get(1)?,
            category: r.get(2)?,
            title: r.get(3)?,
            description: r.get(4)?,
            confidence: r.get(5)?,
            evidence: serde_json::from_str(&r.get::<_, String>(6)?).unwrap_or_default(),
            affected_node_ids: serde_json::from_str(&r.get::<_, String>(7)?).unwrap_or_default(),
            suggestions: serde_json::from_str(&r.get::<_, String>(8)?).unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

pub fn hotspots(conn: &Connection, repository_id: &str, limit: u32) -> Result<Vec<MetricRecord>> {
    let mut stmt = conn.prepare(
        "SELECT m.file_id, f.path, m.fan_in, m.fan_out, m.instability, m.complexity, m.change_frequency, m.hotspot_score
         FROM metrics m JOIN files f ON f.id = m.file_id
         WHERE m.repository_id = ?1 ORDER BY m.hotspot_score DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![repository_id, limit], |r| {
        Ok(MetricRecord {
            file_id: r.get(0)?,
            path: r.get(1)?,
            fan_in: r.get::<_, i64>(2)? as u32,
            fan_out: r.get::<_, i64>(3)? as u32,
            instability: r.get(4)?,
            complexity: r.get::<_, i64>(5)? as u32,
            change_frequency: r.get::<_, i64>(6)? as u32,
            hotspot_score: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// Which module prefixes exist (top-level directories under root).
pub fn modules(conn: &Connection, repository_id: &str) -> Result<Vec<String>> {
    let mut mods: HashSet<String> = HashSet::new();
    let mut stmt = conn.prepare("SELECT path FROM files WHERE repository_id = ?1")?;
    let rows = stmt.query_map(params![repository_id], |r| r.get::<_, String>(0))?;
    for row in rows {
        let path = row?;
        match path.rfind('/') {
            Some(i) => {
                mods.insert(path[..i].to_string());
            }
            None => {
                mods.insert("(root)".to_string());
            }
        }
    }
    let mut v: Vec<String> = mods.into_iter().collect();
    v.sort();
    Ok(v)
}

pub fn change_impact(
    conn: &Connection,
    repository_id: &str,
    file_id: &str,
) -> Result<crate::core::graph::ChangeImpact> {
    let mut edges: Vec<crate::core::types::GraphEdge> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, edge_type, confidence, resolution_method, evidence_path, evidence_line, evidence_description
             FROM graph_edges WHERE repository_id = ?1 AND edge_type='imports'",
        )?;
        let rows = stmt.query_map(params![repository_id], |r| {
            Ok(crate::core::types::GraphEdge {
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
    let mut paths: HashMap<String, String> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT id, path FROM files WHERE repository_id = ?1")?;
        let rows = stmt.query_map(params![repository_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (id, path) = row?;
            paths.insert(id, path);
        }
    }
    let g = DepGraph::build(&edges);
    let label = |id: &str| paths.get(id).cloned().unwrap_or_else(|| id.to_string());
    Ok(crate::core::graph::change_impact(&g, file_id, label))
}
