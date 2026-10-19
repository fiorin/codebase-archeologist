use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    TypeScript,
    JavaScript,
    Tsx,
    Jsx,
    Unknown,
}

impl Language {
    pub fn from_path(path: &str) -> Language {
        let lower = path.to_ascii_lowercase();
        if lower.ends_with(".ts") {
            Language::TypeScript
        } else if lower.ends_with(".tsx") {
            Language::Tsx
        } else if lower.ends_with(".jsx") {
            Language::Jsx
        } else if lower.ends_with(".js") || lower.ends_with(".mjs") || lower.ends_with(".cjs") {
            Language::JavaScript
        } else if lower.ends_with(".mts") || lower.ends_with(".cts") {
            Language::TypeScript
        } else {
            Language::Unknown
        }
    }

    pub fn is_target(&self) -> bool {
        !matches!(self, Language::Unknown)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Language::TypeScript => "typescript",
            Language::JavaScript => "javascript",
            Language::Tsx => "tsx",
            Language::Jsx => "jsx",
            Language::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Module,
    Class,
    Interface,
    Function,
    Method,
    Variable,
    Constant,
    Type,
    Enum,
    Namespace,
    Component,
    Hook,
}

impl SymbolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolKind::Module => "module",
            SymbolKind::Class => "class",
            SymbolKind::Interface => "interface",
            SymbolKind::Function => "function",
            SymbolKind::Method => "method",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Type => "type",
            SymbolKind::Enum => "enum",
            SymbolKind::Namespace => "namespace",
            SymbolKind::Component => "component",
            SymbolKind::Hook => "hook",
        }
    }

    pub fn from_str(s: &str) -> SymbolKind {
        match s {
            "module" => SymbolKind::Module,
            "class" => SymbolKind::Class,
            "interface" => SymbolKind::Interface,
            "function" => SymbolKind::Function,
            "method" => SymbolKind::Method,
            "variable" => SymbolKind::Variable,
            "constant" => SymbolKind::Constant,
            "type" => SymbolKind::Type,
            "enum" => SymbolKind::Enum,
            "namespace" => SymbolKind::Namespace,
            "component" => SymbolKind::Component,
            "hook" => SymbolKind::Hook,
            _ => SymbolKind::Variable,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file_id: String,
    pub path: String,
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: String,
    pub repository_id: String,
    pub path: String,
    pub language: String,
    pub size: u64,
    pub hash: String,
    pub line_count: u32,
    pub parse_status: String,
    pub parse_errors: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolRecord {
    pub id: String,
    pub file_id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub exported: bool,
    pub signature: Option<String>,
    pub complexity: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRecord {
    pub file_id: String,
    pub specifier: String,
    pub kind: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub resolved: bool,
    pub resolved_file_id: Option<String>,
    pub resolution_method: Option<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub file_id: Option<String>,
    pub module: Option<String>,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub confidence: f32,
    pub resolution_method: Option<String>,
    pub evidence: Evidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub kind: String,
    pub file_id: Option<String>,
    pub path: Option<String>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub description: String,
}

impl Evidence {
    pub fn source(
        path: String,
        file_id: String,
        start_line: u32,
        end_line: u32,
        description: String,
    ) -> Self {
        Evidence {
            kind: "source".into(),
            file_id: Some(file_id),
            path: Some(path),
            start_line: Some(start_line),
            end_line: Some(end_line),
            description,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub id: String,
    pub severity: String,
    pub category: String,
    pub title: String,
    pub description: String,
    pub evidence: Vec<Evidence>,
    pub affected_node_ids: Vec<String>,
    pub confidence: f32,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricRecord {
    pub file_id: String,
    pub path: String,
    pub fan_in: u32,
    pub fan_out: u32,
    pub instability: f64,
    pub complexity: u32,
    pub change_frequency: u32,
    pub hotspot_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisRun {
    pub id: String,
    pub repository_id: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub status: String,
    pub files_scanned: u32,
    pub files_parsed: u32,
    pub files_skipped: u32,
    pub symbols: u32,
    pub edges: u32,
    pub findings: u32,
    pub analyzer_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub kind: String,
    pub message: String,
    pub current: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub repository_id: String,
    pub root: String,
    pub name: String,
    pub file_count: u32,
    pub symbol_count: u32,
    pub edge_count: u32,
    pub finding_count: u32,
    pub languages: Vec<LanguageCount>,
    pub last_indexed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageCount {
    pub language: String,
    pub count: u32,
}
