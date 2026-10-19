use codebase_archaeologist_lib::core::db;
use codebase_archaeologist_lib::core::indexer::{IndexJob, Indexer};
use codebase_archaeologist_lib::core::queries;
use codebase_archaeologist_lib::core::types::ProgressEvent;
use rusqlite::Connection;
use std::fs;
use std::path::Path;
use std::process::Command;

fn write(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

/// A small fixture with a deliberate cycle: a -> b -> c -> a
fn build_fixture(root: &Path) {
    write(
        root,
        "src/a.ts",
        "import { b } from './b';\nexport function a(): number { return b() + 1; }\n",
    );
    write(
        root,
        "src/b.ts",
        "import { c, helper } from './c';\nexport function b(): number { if (c() > 0 && helper()) { return 1; } return 2; }\n",
    );
    write(
        root,
        "src/c.ts",
        "import { a } from './a';\nexport function c(): number { return a() - 1; }\nexport const helper = () => true;\n",
    );
    write(
        root,
        "src/util/format.ts",
        "export interface Options { width: number; }\nexport type Formatter = (s: string) => string;\nexport class FormatterImpl {\n  format(s: string): string { return s.trim(); }\n}\n",
    );
    write(
        root,
        "src/index.ts",
        "import { a } from './a';\nimport { FormatterImpl } from './util/format';\nexport const main = () => new FormatterImpl().format(String(a()));\n",
    );
    // File that should be excluded
    write(
        root,
        "node_modules/pkg/index.js",
        "export const nope = 1;\n",
    );
    write(root, ".gitignore", "dist/\n");
    write(root, "dist/bundle.js", "export const built = 1;\n");
}

fn run_index(conn: &mut Connection, root: &Path, mode: &str) {
    let job = IndexJob::new();
    let mut noop = |_: ProgressEvent| {};
    let mut indexer = Indexer::new(conn, root.to_path_buf(), &job, &mut noop);
    indexer.index(mode).expect("index should succeed");
}

fn edge_fingerprint(conn: &Connection, repo_id: &str) -> Vec<(String, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT sf.path, tf.path FROM graph_edges e
             JOIN files sf ON sf.id = e.source_id
             JOIN files tf ON tf.id = e.target_id
             WHERE e.repository_id = ?1 AND e.edge_type = 'imports'
             ORDER BY sf.path, tf.path",
        )
        .unwrap();
    let rows = stmt
        .query_map([repo_id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })
        .unwrap();
    rows.collect::<std::result::Result<Vec<_>, _>>().unwrap()
}

fn symbol_fingerprint(conn: &Connection, repo_id: &str) -> Vec<(String, String, String)> {
    let mut stmt = conn
        .prepare(
            "SELECT f.path, s.qualified_name, s.kind FROM symbols s
             JOIN files f ON f.id = s.file_id
             WHERE f.repository_id = ?1
             ORDER BY f.path, s.qualified_name, s.kind",
        )
        .unwrap();
    let rows = stmt
        .query_map([repo_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .unwrap();
    rows.collect::<std::result::Result<Vec<_>, _>>().unwrap()
}

#[test]
fn scans_extracts_and_resolves() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_fixture(root);

    let mut conn = db::open_memory().unwrap();
    let repo_id = {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(&mut conn, root.to_path_buf(), &job, &mut noop);
        indexer.ensure_repository().unwrap()
    };
    run_index(&mut conn, root, "full");

    let summary = queries::summary(&conn, &repo_id).unwrap();

    // 5 source files indexed; node_modules and dist excluded.
    assert_eq!(summary.file_count, 5, "expected 5 indexed files");

    let files = queries::list_files(&conn, &repo_id, 100, 0).unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"src/a.ts"));
    assert!(!paths.iter().any(|p| p.starts_with("node_modules")));
    assert!(!paths.iter().any(|p| p.starts_with("dist")));

    // Symbols extracted, including the class and interface.
    let symbols = symbol_fingerprint(&conn, &repo_id);
    assert!(
        symbols
            .iter()
            .any(|(p, n, k)| p == "src/util/format.ts" && n == "Options" && k == "interface")
    );
    assert!(
        symbols
            .iter()
            .any(|(p, n, k)| p == "src/util/format.ts" && n == "FormatterImpl" && k == "class")
    );

    // Imports resolved: a.ts -> b.ts, c.ts -> a.ts, index.ts -> a.ts
    let edges = edge_fingerprint(&conn, &repo_id);
    assert!(edges.contains(&("src/a.ts".into(), "src/b.ts".into())));
    assert!(edges.contains(&("src/c.ts".into(), "src/a.ts".into())));
    assert!(edges.contains(&("src/index.ts".into(), "src/util/format.ts".into())));

    // A cycle a -> b -> c -> a must be reported.
    let findings = queries::findings(&conn, &repo_id).unwrap();
    assert!(
        findings.iter().any(|f| f.category == "cycle"),
        "expected a circular dependency finding"
    );
}

#[test]
fn incremental_matches_full_index() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_fixture(root);

    // Reference: single clean full index.
    let mut full_conn = db::open_memory().unwrap();
    let _full_repo = {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(&mut full_conn, root.to_path_buf(), &job, &mut noop);
        indexer.ensure_repository().unwrap()
    };
    run_index(&mut full_conn, root, "full");

    // Incremental path: index, mutate, re-index incrementally.
    let mut inc_conn = db::open_memory().unwrap();
    let inc_repo = {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(&mut inc_conn, root.to_path_buf(), &job, &mut noop);
        indexer.ensure_repository().unwrap()
    };
    run_index(&mut inc_conn, root, "full");

    // Mutate: add a new file, edit an existing one, delete one.
    write(
        root,
        "src/d.ts",
        "import { a } from './a';\nexport const d = () => a() * 2;\n",
    );
    write(
        root,
        "src/b.ts",
        "import { c, helper } from './c';\nimport { d } from './d';\nexport function b(): number { return c() + (helper() ? d() : 0); }\n",
    );
    fs::remove_file(root.join("src/util/format.ts")).unwrap();

    // Re-index incrementally on the mutated tree.
    run_index(&mut inc_conn, root, "incremental");

    // Rebuild a clean full index of the mutated tree in a fresh database.
    let mut ref_conn = db::open_memory().unwrap();
    let ref_repo = {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(&mut ref_conn, root.to_path_buf(), &job, &mut noop);
        indexer.ensure_repository().unwrap()
    };
    run_index(&mut ref_conn, root, "full");

    let inc_edges = edge_fingerprint(&inc_conn, &inc_repo);
    let ref_edges = edge_fingerprint(&ref_conn, &ref_repo);
    assert_eq!(
        inc_edges, ref_edges,
        "incremental index must converge to the same dependency graph as a full index"
    );

    let inc_symbols = symbol_fingerprint(&inc_conn, &inc_repo);
    let ref_symbols = symbol_fingerprint(&ref_conn, &ref_repo);
    assert_eq!(
        inc_symbols, ref_symbols,
        "incremental index must converge to the same symbol set as a full index"
    );

    // The deleted file must be gone.
    let files = queries::list_files(&inc_conn, &inc_repo, 100, 0).unwrap();
    assert!(!files.iter().any(|f| f.path == "src/util/format.ts"));
}

#[test]
fn cycle_detection_ignores_acyclic_graph() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        root,
        "src/x.ts",
        "import { y } from './y';\nexport const x = () => y();\n",
    );
    write(root, "src/y.ts", "export const y = () => 1;\n");

    let mut conn = db::open_memory().unwrap();
    let repo_id = {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(&mut conn, root.to_path_buf(), &job, &mut noop);
        indexer.ensure_repository().unwrap()
    };
    run_index(&mut conn, root, "full");

    let findings = queries::findings(&conn, &repo_id).unwrap();
    assert!(findings.iter().all(|f| f.category != "cycle"));
}

#[test]
fn cancellation_leaves_database_consistent() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    build_fixture(root);

    let mut conn = db::open_memory().unwrap();
    let repo_id = {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(&mut conn, root.to_path_buf(), &job, &mut noop);
        indexer.ensure_repository().unwrap()
    };

    // Complete a clean index first.
    run_index(&mut conn, root, "full");
    let before = edge_fingerprint(&conn, &repo_id);

    // Now attempt an index that is cancelled before it starts work.
    let job = IndexJob::new();
    job.cancel();
    let mut noop = |_: ProgressEvent| {};
    let mut indexer = Indexer::new(&mut conn, root.to_path_buf(), &job, &mut noop);
    let result = indexer.index("full");
    assert!(result.is_err());

    // Existing data must be intact.
    let after = edge_fingerprint(&conn, &repo_id);
    assert_eq!(before, after);
}

#[test]
fn stores_per_symbol_complexity_and_reports_static_findings() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        root,
        "src/quality.ts",
        "import { absent } from './missing';\nexport function simple() { return 1; }\nexport function branchy(x: boolean) { if (x) return 1; if (!x) return 2; return 3; }\n",
    );
    let branches = (0..15)
        .map(|i| format!("if (x === {i}) {{ return {i}; }}"))
        .collect::<Vec<_>>()
        .join("\n");
    write(
        root,
        "src/complex.ts",
        &format!("export function tangled(x: number) {{\n{branches}\nreturn 0;\n}}\n"),
    );
    write(root, "src/broken.ts", "export function broken( {\n");
    let mut hub_imports = String::new();
    for i in 0..10 {
        write(root, &format!("src/dep{i}.ts"), "export const value = 1;\n");
        hub_imports.push_str(&format!("import './dep{i}';\n"));
    }
    write(root, "src/hub.ts", &hub_imports);

    let mut conn = db::open_memory().unwrap();
    let repo_id = {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(&mut conn, root.to_path_buf(), &job, &mut noop);
        indexer.ensure_repository().unwrap()
    };
    run_index(&mut conn, root, "full");

    let symbol_complexities: Vec<(String, Option<i64>)> = {
        let mut stmt = conn
            .prepare("SELECT name, complexity FROM symbols WHERE name IN ('simple', 'branchy') ORDER BY name")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    };
    assert_eq!(
        symbol_complexities,
        vec![("branchy".into(), Some(3)), ("simple".into(), Some(1))]
    );

    let findings = queries::findings(&conn, &repo_id).unwrap();
    assert!(
        findings
            .iter()
            .any(|f| f.category == "complexity" && f.title.contains("tangled"))
    );
    assert!(findings.iter().any(|f| f.category == "unresolved_import"));
    assert!(
        findings
            .iter()
            .any(|f| f.category == "coupling" && f.title.contains("hub.ts"))
    );
    assert!(
        findings
            .iter()
            .any(|f| f.category == "parse_error" && f.title.contains("broken.ts"))
    );
    assert!(
        findings
            .iter()
            .filter(|f| f.category != "coupling")
            .all(|f| !f.evidence.is_empty())
    );
}

#[test]
fn git_history_contributes_to_file_churn() {
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(root, "src/changed.ts", "export const value = 1;\n");

    let git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-q"]);
    git(&["config", "user.name", "Archaeologist Test"]);
    git(&["config", "user.email", "archaeologist@example.test"]);
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "initial"]);
    write(root, "src/changed.ts", "export const value = 2;\n");
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "change"]);

    let mut conn = db::open_memory().unwrap();
    let repo_id = {
        let job = IndexJob::new();
        let mut noop = |_: ProgressEvent| {};
        let mut indexer = Indexer::new(&mut conn, root.to_path_buf(), &job, &mut noop);
        indexer.ensure_repository().unwrap()
    };
    run_index(&mut conn, root, "full");

    let churn: u32 = conn
        .query_row(
            "SELECT m.change_frequency FROM metrics m JOIN files f ON f.id=m.file_id
             WHERE m.repository_id=?1 AND f.path='src/changed.ts'",
            [&repo_id],
            |r| r.get::<_, i64>(0).map(|v| v as u32),
        )
        .unwrap();
    assert_eq!(
        churn, 2,
        "initial addition and subsequent modification are two changes"
    );
}
