use super::{Extraction, LanguageAnalyzer, RawCall, RawExport, RawImport, RawSymbol};
use crate::core::types::{Language, SymbolKind};
use tree_sitter::{Node, Parser};

pub struct TypeScriptAnalyzer {
    language: Language,
    ts_language: tree_sitter::Language,
}

impl TypeScriptAnalyzer {
    pub fn new(language: Language) -> Self {
        let ts_language = match language {
            Language::Tsx | Language::Jsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            _ => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        };
        TypeScriptAnalyzer {
            language,
            ts_language,
        }
    }
}

impl LanguageAnalyzer for TypeScriptAnalyzer {
    fn language(&self) -> Language {
        self.language
    }

    fn extract(&self, source: &str) -> Extraction {
        let mut out = Extraction::default();
        let mut parser = Parser::new();
        if parser.set_language(&self.ts_language).is_err() {
            return out;
        }
        let Some(tree) = parser.parse(source, None) else {
            return out;
        };
        let root = tree.root_node();

        let mut ctx = Ctx {
            src: source.as_bytes(),
            out: &mut out,
            stack: Vec::new(),
            language: self.language,
        };

        ctx.count_errors(root);
        ctx.visit_block(root);
        ctx.mark_exported(root);

        out.max_complexity = out.max_complexity.max(1);
        out
    }
}

struct Ctx<'a> {
    src: &'a [u8],
    out: &'a mut Extraction,
    stack: Vec<String>,
    language: Language,
}

impl<'a> Ctx<'a> {
    fn text(&self, node: Node) -> &'a str {
        node.utf8_text(self.src).unwrap_or("")
    }

    fn count_errors(&mut self, node: Node) {
        if node.is_error() || node.is_missing() {
            self.out.parse_errors += 1;
        }
        for i in 0..node.child_count() {
            if let Some(c) = node.child(i) {
                self.count_errors(c);
            }
        }
    }

    fn span(node: Node) -> (u32, u32, u32, u32) {
        let s = node.start_position();
        let e = node.end_position();
        (
            s.row as u32 + 1,
            s.column as u32,
            e.row as u32 + 1,
            e.column as u32,
        )
    }

    fn qualified(&self, name: &str) -> String {
        if self.stack.is_empty() {
            name.to_string()
        } else {
            format!("{}.{}", self.stack.join("."), name)
        }
    }

    fn push_symbol(
        &mut self,
        node: Node,
        name: &str,
        kind: SymbolKind,
        exported: bool,
        signature: Option<String>,
        complexity: Option<u32>,
    ) {
        let (sl, sc, el, ec) = Self::span(node);
        let qualified = self.qualified(name);
        let symbol = RawSymbol {
            name: name.to_string(),
            qualified_name: qualified.clone(),
            kind,
            start_line: sl,
            start_column: sc,
            end_line: el,
            end_column: ec,
            exported,
            signature,
            parent_path: self.stack.last().cloned(),
            complexity,
        };
        self.out.symbols.push(symbol);
        self.stack.push(qualified);
    }

    fn pop(&mut self) {
        self.stack.pop();
    }

    fn visit_block(&mut self, node: Node) {
        for i in 0..node.named_child_count() as u32 {
            if let Some(child) = node.named_child(i) {
                self.visit(child);
            }
        }
    }

    fn visit(&mut self, node: Node) {
        match node.kind() {
            "import_statement" => {
                self.record_import(node);
                return;
            }
            "export_statement" => {
                self.record_export_statement(node);
                return;
            }
            "call_expression" => {
                self.record_call(node);
            }
            "function_declaration"
            | "generator_function_declaration"
            | "function_signature"
            | "abstract_method_signature" => {
                return self.record_function_like(node);
            }
            "class_declaration" | "abstract_class_declaration" | "class" => {
                return self.record_class(node);
            }
            "interface_declaration" => {
                return self.record_interface(node);
            }
            "type_alias_declaration" => {
                return self.record_type_alias(node);
            }
            "enum_declaration" => {
                return self.record_enum(node);
            }
            "internal_module" | "module" => {
                return self.record_module(node);
            }
            "lexical_declaration" | "variable_declaration" | "ambient_declaration" => {
                return self.record_variable_container(node);
            }
            "method_definition" | "method_signature" => {
                return self.record_method(node);
            }
            _ => {}
        }

        self.visit_block(node);
    }

    fn record_function_like(&mut self, node: Node) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n).to_string())
            .unwrap_or_else(|| "(anonymous)".to_string());

        let exported = false;
        let kind = classify_callable(&name, self.language);
        let signature = self.signature_up_to_body(node);
        let complexity = complexity_of(node, self.src);
        self.push_symbol(node, &name, kind, exported, signature, Some(complexity));
        self.out.max_complexity = self.out.max_complexity.max(complexity);

        // Parameters are their own symbols? Keep them out; visit body only.
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_block(body);
        } else {
            self.visit_block(node);
        }
        self.pop();
    }

    fn record_method(&mut self, node: Node) {
        let name = node
            .child_by_field_name("name")
            .map(|n| strip_accessor(self.text(n)))
            .unwrap_or_else(|| "(anonymous)".to_string());
        let kind = if name.starts_with("constructor") {
            SymbolKind::Method
        } else {
            classify_callable(&name, self.language)
        };
        let signature = self.signature_up_to_body(node);
        let complexity = complexity_of(node, self.src);
        self.push_symbol(node, &name, kind, false, signature, Some(complexity));
        self.out.max_complexity = self.out.max_complexity.max(complexity);
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_block(body);
        }
        self.pop();
    }

    fn record_class(&mut self, node: Node) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n).to_string())
            .unwrap_or_else(|| "(anonymous)".to_string());
        self.push_symbol(node, &name, SymbolKind::Class, false, None, None);
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_block(body);
        }
        self.pop();
    }

    fn record_interface(&mut self, node: Node) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n).to_string())
            .unwrap_or_default();
        self.push_symbol(node, &name, SymbolKind::Interface, false, None, None);
        if let Some(body) = node.child_by_field_name("body") {
            self.visit_block(body);
        }
        self.pop();
    }

    fn record_type_alias(&mut self, node: Node) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n).to_string())
            .unwrap_or_default();
        let sig = self.text(node).chars().take(200).collect::<String>();
        self.push_symbol(node, &name, SymbolKind::Type, false, Some(sig), None);
        self.visit_block(node);
        self.pop();
    }

    fn record_enum(&mut self, node: Node) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n).to_string())
            .unwrap_or_default();
        self.push_symbol(node, &name, SymbolKind::Enum, false, None, None);
        self.visit_block(node);
        self.pop();
    }

    fn record_module(&mut self, node: Node) {
        let name = node
            .child_by_field_name("name")
            .map(|n| self.text(n).to_string())
            .unwrap_or_default();
        if name.is_empty() {
            self.visit_block(node);
            return;
        }
        self.push_symbol(node, &name, SymbolKind::Namespace, false, None, None);
        self.visit_block(node);
        self.pop();
    }

    fn record_variable_container(&mut self, node: Node) {
        // `const x = () => {}` and friends: walk declarators.
        for i in 0..node.named_child_count() as u32 {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() == "variable_declarator" {
                self.record_declarator(child, node);
            } else {
                self.visit(child);
            }
        }
    }

    fn record_declarator(&mut self, node: Node, parent: Node) {
        let Some(name_node) = node.child_by_field_name("name") else {
            self.visit_block(node);
            return;
        };
        let raw_name = self.text(name_node).trim().to_string();
        if raw_name.is_empty() || raw_name.contains('{') {
            self.visit_block(node);
            return;
        }

        let is_const = self.text(parent).trim_start().starts_with("const");

        let value_is_callable = node
            .child_by_field_name("value")
            .map(|v| {
                matches!(
                    v.kind(),
                    "arrow_function" | "function_expression" | "function"
                )
            })
            .unwrap_or(false);

        let kind = if value_is_callable {
            classify_callable(&raw_name, self.language)
        } else if is_const {
            SymbolKind::Constant
        } else {
            SymbolKind::Variable
        };

        let signature = if value_is_callable {
            node.child_by_field_name("value")
                .map(|v| first_line(self.text(v)))
        } else {
            None
        };

        let complexity = if value_is_callable {
            node.child_by_field_name("value")
                .map(|v| complexity_of(v, self.src))
        } else {
            None
        };
        self.push_symbol(node, &raw_name, kind, false, signature, complexity);

        if let Some(complexity) = complexity {
            self.out.max_complexity = self.out.max_complexity.max(complexity);
        }

        self.visit_block(node);
        self.pop();
    }

    fn signature_up_to_body(&self, node: Node) -> Option<String> {
        let body = node.child_by_field_name("body");
        let text = self.text(node);
        let end = body
            .map(|b| b.start_byte() - node.start_byte())
            .unwrap_or(text.len().min(300));
        let sliced = &text[..end.min(text.len())];
        Some(first_line(sliced))
    }

    fn record_import(&mut self, node: Node) {
        let Some(source) = node.child_by_field_name("source") else {
            return;
        };
        let specifier = strip_quotes(self.text(source));
        if specifier.is_empty() {
            return;
        }
        let is_type_only = self.text(node).trim_start().starts_with("import type");
        let (sl, sc, el, ec) = Self::span(source);
        self.out.imports.push(RawImport {
            specifier,
            kind: if is_type_only { "type" } else { "value" }.into(),
            start_line: sl,
            start_column: sc,
            end_line: el,
            end_column: ec,
            is_type_only,
        });
    }

    fn record_export_statement(&mut self, node: Node) {
        let text = self.text(node);

        // export * from 'x' / export { a } from 'x'  -> dependency too
        if let Some(source) = node.child_by_field_name("source") {
            let specifier = strip_quotes(self.text(source));
            if !specifier.is_empty() {
                let (sl, sc, el, ec) = Self::span(source);
                self.out.imports.push(RawImport {
                    specifier,
                    kind: "re-export".into(),
                    start_line: sl,
                    start_column: sc,
                    end_line: el,
                    end_column: ec,
                    is_type_only: false,
                });
            }
            // named re-exports become exports as well
            if let Some(clause) = node.child_by_field_name("declaration") {
                let _ = clause;
            }
        }

        // export { a, b as c }
        for i in 0..node.named_child_count() as u32 {
            let Some(child) = node.named_child(i) else {
                continue;
            };
            if child.kind() == "export_clause" {
                for j in 0..child.named_child_count() as u32 {
                    let Some(spec) = child.named_child(j) else {
                        continue;
                    };
                    if spec.kind() != "export_specifier" {
                        continue;
                    }
                    let alias = spec.child_by_field_name("alias");
                    let name_node = spec.child_by_field_name("name");
                    let exported_name = alias.or(name_node).map(|n| self.text(n).to_string());
                    let original = name_node.map(|n| self.text(n).to_string());
                    if let Some(orig) = original {
                        let line = spec.start_position().row as u32 + 1;
                        self.out.exports.push(RawExport {
                            name: exported_name.unwrap_or(orig.clone()),
                            kind: "named".into(),
                            line,
                        });
                    }
                }
            }
        }

        // `export default ...`
        if text.trim_start().starts_with("export default") {
            let line = node.start_position().row as u32 + 1;
            self.out.exports.push(RawExport {
                name: "default".into(),
                kind: "default".into(),
                line,
            });
        }

        // Walk the declaration, marking everything inside as exported.
        if let Some(decl) = node.child_by_field_name("declaration") {
            let before = self.out.symbols.len();
            self.visit(decl);
            for s in self.out.symbols.iter_mut().skip(before) {
                s.exported = true;
            }
        } else {
            // `export const foo = ...` where declaration is a lexical_declaration child
            for i in 0..node.named_child_count() as u32 {
                let Some(child) = node.named_child(i) else {
                    continue;
                };
                match child.kind() {
                    "lexical_declaration"
                    | "variable_declaration"
                    | "function_declaration"
                    | "class_declaration"
                    | "interface_declaration"
                    | "type_alias_declaration"
                    | "enum_declaration"
                    | "internal_module"
                    | "generator_function_declaration"
                    | "ambient_declaration" => {
                        let before = self.out.symbols.len();
                        self.visit(child);
                        for s in self.out.symbols.iter_mut().skip(before) {
                            s.exported = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn mark_exported(&mut self, root: Node) {
        // Names appearing in `export { x }` (without `from`) mark the local symbol.
        let mut names: Vec<String> = Vec::new();
        let mut cursor = root.walk();
        collect_exported_names(root, self.src, &mut names, &mut cursor);
        if names.is_empty() {
            return;
        }
        for symbol in self.out.symbols.iter_mut() {
            if !symbol.exported && names.iter().any(|n| n == &symbol.name) {
                symbol.exported = true;
            }
        }
    }

    fn record_call(&mut self, node: Node) {
        let Some(func) = node.child_by_field_name("function") else {
            return;
        };
        let func_text = self.text(func).trim().to_string();
        let line = node.start_position().row as u32 + 1;

        // Dynamic import / require
        if func_text == "import" || func_text == "require" {
            if let Some(args) = node.child_by_field_name("arguments") {
                if let Some(first) = args.named_child(0) {
                    if first.kind() == "string" {
                        let specifier = strip_quotes(self.text(first));
                        if !specifier.is_empty() {
                            let (sl, sc, el, ec) = Self::span(first);
                            self.out.imports.push(RawImport {
                                specifier,
                                kind: if func_text == "import" {
                                    "dynamic".into()
                                } else {
                                    "require".into()
                                },
                                start_line: sl,
                                start_column: sc,
                                end_line: el,
                                end_column: ec,
                                is_type_only: false,
                            });
                        }
                    }
                }
            }
            return;
        }

        let callee = member_path(func, self.src);
        if !callee.is_empty() {
            self.out.calls.push(RawCall {
                callee,
                start_line: line,
                confidence: 0.35,
            });
        }
    }
}

fn collect_exported_names<'a>(
    node: Node<'a>,
    src: &'a [u8],
    out: &mut Vec<String>,
    _cursor: &mut tree_sitter::TreeCursor<'a>,
) {
    if node.kind() == "export_clause" {
        for i in 0..node.named_child_count() as u32 {
            if let Some(spec) = node.named_child(i) {
                if spec.kind() == "export_specifier" {
                    if let Some(name) = spec.child_by_field_name("name") {
                        if let Ok(t) = name.utf8_text(src) {
                            out.push(t.to_string());
                        }
                    }
                }
            }
        }
    }
    for i in 0..node.named_child_count() as u32 {
        if let Some(c) = node.named_child(i) {
            collect_exported_names(c, src, out, _cursor);
        }
    }
}

fn classify_callable(name: &str, language: Language) -> SymbolKind {
    let tsx_like = matches!(language, Language::Tsx | Language::Jsx);
    if name.starts_with("use") && name.len() > 3 && name.as_bytes()[3].is_ascii_uppercase() {
        return SymbolKind::Hook;
    }
    if tsx_like
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_uppercase())
            .unwrap_or(false)
    {
        return SymbolKind::Component;
    }
    SymbolKind::Function
}

fn strip_accessor(name: &str) -> String {
    name.trim_start_matches("static ")
        .trim_start_matches("async ")
        .trim_start_matches("get ")
        .trim_start_matches("set ")
        .trim()
        .to_string()
}

fn strip_quotes(s: &str) -> String {
    let t = s.trim();
    let bytes = t.as_bytes();
    if bytes.len() >= 2 {
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'"' || first == b'\'' || first == b'`') && first == last {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

fn first_line(s: &str) -> String {
    let line = s.lines().next().unwrap_or("").trim().to_string();
    if line.len() > 300 {
        format!("{}…", &line[..300])
    } else {
        line
    }
}

fn member_path(node: Node, src: &[u8]) -> String {
    match node.kind() {
        "identifier" | "property_identifier" => node.utf8_text(src).unwrap_or("").to_string(),
        "member_expression" | "subscript_expression" | "optional_chain" => {
            let obj = node.child_by_field_name("object");
            let prop = node
                .child_by_field_name("property")
                .or_else(|| node.child_by_field_name("index"));
            let o = obj.map(|n| member_path(n, src)).unwrap_or_default();
            let p = prop.map(|n| member_path(n, src)).unwrap_or_default();
            if o.is_empty() {
                p
            } else if p.is_empty() {
                o
            } else {
                format!("{o}.{p}")
            }
        }
        "call_expression" => node
            .child_by_field_name("function")
            .map(|n| member_path(n, src))
            .unwrap_or_default(),
        "new_expression" => node
            .child_by_field_name("constructor")
            .map(|n| member_path(n, src))
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn complexity_of(node: Node, src: &[u8]) -> u32 {
    let mut score: u32 = 1;
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "if_statement" | "for_statement" | "for_in_statement" | "while_statement"
            | "do_statement" | "catch_clause" | "ternary_expression" | "switch_case" => {
                score += 1;
            }
            "binary_expression" => {
                let text = n.utf8_text(src).unwrap_or("");
                if text.contains("&&") || text.contains("||") || text.contains("??") {
                    score += 1;
                }
            }
            _ => {}
        }
        for i in 0..n.named_child_count() as u32 {
            if let Some(c) = n.named_child(i) {
                stack.push(c);
            }
        }
    }
    score
}
