use crate::core::types::{Language, SymbolKind};

#[derive(Debug, Clone)]
pub struct RawSymbol {
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub exported: bool,
    pub signature: Option<String>,
    pub parent_path: Option<String>,
    pub complexity: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct RawImport {
    pub specifier: String,
    pub kind: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub is_type_only: bool,
}

#[derive(Debug, Clone)]
pub struct RawExport {
    pub name: String,
    pub kind: String,
    pub line: u32,
}

#[derive(Debug, Clone)]
pub struct RawCall {
    pub callee: String,
    pub start_line: u32,
    pub confidence: f32,
}

#[derive(Debug, Default)]
pub struct Extraction {
    pub symbols: Vec<RawSymbol>,
    pub imports: Vec<RawImport>,
    pub exports: Vec<RawExport>,
    pub calls: Vec<RawCall>,
    pub parse_errors: u32,
    pub max_complexity: u32,
}

pub trait LanguageAnalyzer: Send + Sync {
    fn language(&self) -> Language;
    fn extract(&self, source: &str) -> Extraction;
}

pub mod typescript;
pub use typescript::TypeScriptAnalyzer;

pub fn analyzer_for(lang: Language) -> Option<Box<dyn LanguageAnalyzer>> {
    match lang {
        Language::TypeScript | Language::Tsx => Some(Box::new(TypeScriptAnalyzer::new(lang))),
        Language::JavaScript | Language::Jsx => Some(Box::new(TypeScriptAnalyzer::new(lang))),
        Language::Unknown => None,
    }
}
