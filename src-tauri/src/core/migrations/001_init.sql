PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS repositories (
    id          TEXT PRIMARY KEY,
    root_path   TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    last_indexed TEXT
);

CREATE TABLE IF NOT EXISTS analysis_runs (
    id              TEXT PRIMARY KEY,
    repository_id   TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    status          TEXT NOT NULL DEFAULT 'running',
    mode            TEXT NOT NULL DEFAULT 'full',
    files_scanned   INTEGER NOT NULL DEFAULT 0,
    files_parsed    INTEGER NOT NULL DEFAULT 0,
    files_skipped   INTEGER NOT NULL DEFAULT 0,
    symbols         INTEGER NOT NULL DEFAULT 0,
    edges           INTEGER NOT NULL DEFAULT 0,
    findings        INTEGER NOT NULL DEFAULT 0,
    analyzer_version TEXT NOT NULL,
    error           TEXT
);
CREATE INDEX IF NOT EXISTS idx_runs_repo ON analysis_runs(repository_id, started_at DESC);

CREATE TABLE IF NOT EXISTS files (
    id              TEXT PRIMARY KEY,
    repository_id   TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    path            TEXT NOT NULL,
    language        TEXT NOT NULL,
    size            INTEGER NOT NULL,
    mtime           INTEGER NOT NULL,
    hash            TEXT NOT NULL,
    line_count      INTEGER NOT NULL DEFAULT 0,
    parse_status    TEXT NOT NULL DEFAULT 'pending',
    parse_errors    INTEGER NOT NULL DEFAULT 0,
    analyzer_version TEXT NOT NULL DEFAULT '',
    indexed_at      TEXT NOT NULL,
    UNIQUE(repository_id, path)
);
CREATE INDEX IF NOT EXISTS idx_files_repo ON files(repository_id);
CREATE INDEX IF NOT EXISTS idx_files_hash ON files(repository_id, hash);
CREATE INDEX IF NOT EXISTS idx_files_path ON files(repository_id, path);

CREATE TABLE IF NOT EXISTS symbols (
    id              TEXT PRIMARY KEY,
    file_id         TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    parent_id       TEXT,
    name            TEXT NOT NULL,
    qualified_name  TEXT NOT NULL,
    kind            TEXT NOT NULL,
    start_line      INTEGER NOT NULL,
    start_column    INTEGER NOT NULL,
    end_line        INTEGER NOT NULL,
    end_column      INTEGER NOT NULL,
    exported        INTEGER NOT NULL DEFAULT 0,
    signature       TEXT,
    complexity      INTEGER,
    UNIQUE(file_id, qualified_name, start_line, kind)
);
CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_qualified ON symbols(qualified_name);
CREATE INDEX IF NOT EXISTS idx_symbols_parent ON symbols(parent_id);

CREATE TABLE IF NOT EXISTS imports (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id         TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    specifier       TEXT NOT NULL,
    kind            TEXT NOT NULL,
    start_line      INTEGER NOT NULL,
    start_column    INTEGER NOT NULL,
    end_line        INTEGER NOT NULL,
    end_column      INTEGER NOT NULL,
    resolved        INTEGER NOT NULL DEFAULT 0,
    resolved_file_id TEXT,
    resolution_method TEXT,
    confidence      REAL NOT NULL DEFAULT 0.0
);
CREATE INDEX IF NOT EXISTS idx_imports_file ON imports(file_id);
CREATE INDEX IF NOT EXISTS idx_imports_resolved ON imports(resolved_file_id);

CREATE TABLE IF NOT EXISTS calls (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id         TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    caller_symbol   TEXT,
    callee          TEXT NOT NULL,
    start_line      INTEGER NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.3
);
CREATE INDEX IF NOT EXISTS idx_calls_file ON calls(file_id);
CREATE INDEX IF NOT EXISTS idx_calls_callee ON calls(callee);

CREATE TABLE IF NOT EXISTS graph_nodes (
    id              TEXT PRIMARY KEY,
    repository_id   TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    kind            TEXT NOT NULL,
    label           TEXT NOT NULL,
    file_id         TEXT,
    module          TEXT,
    data            TEXT NOT NULL DEFAULT '{}',
    UNIQUE(repository_id, id)
);
CREATE INDEX IF NOT EXISTS idx_gn_repo_kind ON graph_nodes(repository_id, kind);

CREATE TABLE IF NOT EXISTS graph_edges (
    id              TEXT PRIMARY KEY,
    repository_id   TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    source_id       TEXT NOT NULL,
    target_id       TEXT NOT NULL,
    edge_type       TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 1.0,
    resolution_method TEXT,
    evidence_path   TEXT,
    evidence_line   INTEGER,
    evidence_description TEXT NOT NULL DEFAULT '',
    UNIQUE(repository_id, source_id, target_id, edge_type)
);
CREATE INDEX IF NOT EXISTS idx_ge_source ON graph_edges(repository_id, source_id);
CREATE INDEX IF NOT EXISTS idx_ge_target ON graph_edges(repository_id, target_id);
CREATE INDEX IF NOT EXISTS idx_ge_type ON graph_edges(repository_id, edge_type);

CREATE TABLE IF NOT EXISTS findings (
    id              TEXT PRIMARY KEY,
    repository_id   TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    analysis_run_id TEXT NOT NULL REFERENCES analysis_runs(id) ON DELETE CASCADE,
    severity        TEXT NOT NULL,
    category        TEXT NOT NULL,
    title           TEXT NOT NULL,
    description     TEXT NOT NULL,
    confidence      REAL NOT NULL DEFAULT 0.5,
    evidence        TEXT NOT NULL DEFAULT '[]',
    affected_nodes  TEXT NOT NULL DEFAULT '[]',
    suggestions     TEXT NOT NULL DEFAULT '[]'
);
CREATE INDEX IF NOT EXISTS idx_findings_repo ON findings(repository_id, category);

CREATE TABLE IF NOT EXISTS metrics (
    repository_id   TEXT NOT NULL REFERENCES repositories(id) ON DELETE CASCADE,
    file_id         TEXT NOT NULL REFERENCES files(id) ON DELETE CASCADE,
    fan_in          INTEGER NOT NULL DEFAULT 0,
    fan_out         INTEGER NOT NULL DEFAULT 0,
    instability     REAL NOT NULL DEFAULT 0.0,
    complexity      INTEGER NOT NULL DEFAULT 0,
    change_frequency INTEGER NOT NULL DEFAULT 0,
    hotspot_score   REAL NOT NULL DEFAULT 0.0,
    PRIMARY KEY(repository_id, file_id)
);

CREATE TABLE IF NOT EXISTS settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL
);

-- Full-text search over symbol names and paths.
CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
    file_id UNINDEXED,
    repository_id UNINDEXED,
    path,
    symbol_name,
    qualified_name,
    content=''
);
