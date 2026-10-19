# Codebase Archaeologist — Product & Implementation Plan

> **One-line vision:** A local-first software archaeology platform that reconstructs the architecture and evolution of existing codebases using syntax analysis, dependency graphs, Git history, static analysis, and AI-assisted reasoning.

## 1. Product definition

Codebase Archaeologist analyzes an existing source repository and builds a navigable, queryable model of its structure, dependencies, history, complexity, and potential architectural problems.

It should answer questions such as:

- What is the architecture of this codebase?
- What are its main modules, and how do they depend on each other?
- How does a specific feature or request flow through the system?
- Which files are architectural hotspots?
- Where are circular dependencies or potential architectural violations?
- Which parts of the system may be risky to change?
- Which files tend to change together despite having no direct dependency?
- How has the architecture evolved over time?
- What would likely break if a symbol, module, or interface changed?
- Can the system explain a subsystem using concrete source evidence?

### Product principles

1. **Deterministic analysis first.** Parse and model the code before asking an LLM to interpret it.
2. **Evidence-backed conclusions.** Findings and AI explanations should link to source locations, graph relationships, tests, or Git history.
3. **Local-first by default.** Repository contents and history stay local unless the user explicitly configures an AI provider and submits context.
4. **Incremental processing.** Re-analyze changed files rather than rebuilding the entire index after every change.
5. **Progressive exploration.** Never render the entire graph of a large repository at once.
6. **AI is optional.** The core scanner, graph, metrics, findings, CLI, and exports work without an LLM.
7. **No destructive automation.** The app may recommend changes, but must not modify or delete source code without explicit user action.

## 2. Initial scope and non-goals

### MVP scope

- Desktop application for Windows, macOS, and Linux, subject to Tauri support.
- TypeScript and JavaScript repositories.
- Repository scanning and ignore rules.
- Syntax-aware extraction of files, symbols, imports, exports, and selected call relationships.
- Persistent SQLite index.
- File/symbol search.
- Interactive dependency graph.
- Initial metrics, circular dependency detection, and hotspot ranking.
- Git history indexing and basic change-coupling analysis after the core scanner is stable.
- Optional AI integration as a later phase.

### Explicit non-goals for the first release

- Replacing an IDE or debugger.
- Fully understanding runtime behavior through static analysis alone.
- Automatically refactoring or modifying source code.
- Supporting every language from day one.
- Running arbitrary repository scripts during indexing.
- Claiming that inferred architecture or dead-code candidates are facts.
- Requiring a hosted AI service.
- Indexing every file as an embedding by default.

## 3. Recommended technology stack

| Area | Choice | Rationale |
|---|---|---|
| Desktop shell | Tauri 2 | Lightweight desktop distribution and Rust integration |
| Frontend | React + TypeScript + Vite | Familiar ecosystem and rapid UI iteration |
| Graph UI | React Flow | Interactive, expandable dependency and architecture graphs |
| Styling | Tailwind CSS | Consistent UI with fast implementation |
| Frontend data fetching | TanStack Query | Async operations, cache state, and loading/error handling |
| Core engine | Rust | Efficient filesystem traversal, parsing orchestration, graph analysis, and CLI reuse |
| Parsing | Tree-sitter | Error-tolerant syntax trees and multi-language extensibility |
| Initial language adapters | TypeScript and JavaScript | High-value initial coverage; add other languages through adapters |
| Storage | SQLite | Local persistence, portability, and transactional updates |
| Text search | SQLite FTS5 initially | Simple local full-text search without another service |
| Git integration | Git CLI initially | Leverage installed Git and avoid implementing Git object parsing in the MVP |
| AI providers | Provider abstraction; OpenAI-compatible API and Ollama | Hosted and local inference options |
| CLI | Rust CLI sharing the same core | Automation, CI usage, and scriptability |
| Export | Markdown, JSON, CSV, SVG/PNG, Mermaid | Reports, integrations, and documentation |

### Stack decisions

- Keep repository scanning, parsing, graph construction, metrics, Git processing, and storage in Rust.
- Keep React focused on presentation and user interaction; do not perform repository-wide analysis in the browser.
- Use Tauri IPC commands for requests and events for long-running progress.
- Start with SQLite FTS5. Evaluate Tantivy only if measured search performance or query requirements justify it.
- Use the Git CLI initially. Wrap it behind an interface so a future library-based implementation can replace it.
- Use Tree-sitter for syntax extraction, but add language-specific resolution logic where imports, aliases, project references, and module semantics require it.
- Do not introduce Neo4j or a separate graph database in the MVP. The graph is stored as normalized relational data and materialized in memory for analysis when useful.
- Do not require a vector database for the first AI release. Start with symbol search, text search, graph traversal, tests, and Git evidence. Add embeddings only if evaluation demonstrates a meaningful retrieval improvement.

## 4. High-level architecture

```text
React + TypeScript UI
        │
        │ Tauri commands / events
        ▼
Rust application layer
        │
        ├── Repository scanner
        ├── Ignore/configuration resolver
        ├── Language analyzer registry
        │      ├── TypeScript / JavaScript
        │      └── Future language adapters
        ├── Symbol and dependency resolver
        ├── Graph model
        ├── Metrics and findings engine
        ├── Git archaeology engine
        ├── Search service
        ├── SQLite persistence
        ├── AI context builder and provider adapters
        └── Exporters
                │
                ▼
             SQLite
```

### Core processing pipeline

```text
Open repository
    ↓
Resolve ignore rules and configuration
    ↓
Discover candidate source files
    ↓
Hash files and compare with indexed state
    ↓
Parse new or changed files
    ↓
Extract symbols, imports, exports, and calls
    ↓
Resolve local references and dependency edges
    ↓
Persist index transactionally
    ↓
Run metrics and graph analyses
    ↓
Persist findings and analysis run
    ↓
Update UI incrementally
```

### Background work

Long-running operations must not block the UI. Use background tasks and emit progress events such as:

- `index.started`
- `index.progress`
- `index.file.completed`
- `index.completed`
- `analysis.started`
- `analysis.completed`
- `git-index.progress`

Support cancellation for indexing and analysis jobs. Ensure cancelled jobs leave the database in a consistent state.

## 5. Repository scanning and incremental indexing

### File discovery

- Respect `.gitignore`, `.git/info/exclude`, and the configured global Git ignore where practical.
- Support repository-specific include/exclude patterns.
- Exclude common generated or dependency directories by default: `node_modules`, `.git`, `dist`, `build`, `target`, `coverage`, `.next`, `.cache`, `vendor`, `bin`, and `obj`.
- Do not follow symlinks outside the repository root by default.
- Avoid parsing binary files.
- Provide a UI to inspect and override exclusions.

### Incremental indexing algorithm

For each file, persist:

- Relative path
- Language
- File size
- Modification time
- Content hash
- Parser version
- Analyzer version
- Last indexed timestamp
- Index status and parse errors

Algorithm:

1. Discover files.
2. Compare path, size, modification time, and content hash with the stored index.
3. Skip files whose content hash and analyzer versions are unchanged.
4. Parse added or changed files.
5. Replace the changed file's symbols and outgoing relationships transactionally.
6. Re-resolve affected imports and references, including files that depend on a changed export.
7. Remove stale records for deleted or renamed files.
8. Recompute only affected graph regions and metrics where feasible; use a full analysis as a correctness fallback.
9. Record an analysis run and emit progress.

**Correctness requirement:** an incremental index must converge to the same logical graph and findings as a clean full index of the same repository and configuration.

### Scale target

Use **100,000 files and 10 million lines of code as aspirational performance targets**, not initial acceptance guarantees. Benchmark on representative repositories and publish measured results. The UI must remain responsive on large repositories through pagination, virtualization, graph expansion, and background jobs.

## 6. Language analyzer contract

Implement a registry of language adapters. Each adapter should expose operations equivalent to:

```rust
trait LanguageAnalyzer {
    fn language(&self) -> Language;
    fn parse(&self, source: &str) -> ParseResult;
    fn extract_symbols(&self, tree: &SyntaxTree) -> Vec<Symbol>;
    fn extract_imports(&self, tree: &SyntaxTree) -> Vec<Import>;
    fn extract_exports(&self, tree: &SyntaxTree) -> Vec<Export>;
    fn extract_calls(&self, tree: &SyntaxTree) -> Vec<Call>;
    fn extract_inheritance(&self, tree: &SyntaxTree) -> Vec<Inheritance>;
}
```

This is conceptual pseudocode; adapt it to the chosen Rust Tree-sitter bindings and concrete syntax tree types.

### Initial extraction requirements

- Functions, methods, classes, interfaces, types, enums, constants, variables, modules/namespaces, and framework-relevant components where detectable.
- Imports and exports.
- Parent/child symbol relationships.
- Source spans: file, start line/column, end line/column.
- Calls that can be identified syntactically.
- Parse errors and partial extraction status.
- Tests and generated-file heuristics.

### Resolution caveats

- Distinguish syntax-level calls from confidently resolved call edges.
- Resolve relative imports and common TypeScript path aliases.
- Read workspace and project configuration where feasible, including `tsconfig` paths and monorepo project manifests.
- Treat dynamic imports, reflection, dependency injection, runtime registration, and generated code as uncertain.
- Store edge confidence and resolution method.
- Never label an incomplete call graph as a complete execution trace.

## 7. Core domain model

### Main entities

- Repository
- File
- Symbol
- Package
- Module
- Graph node and edge
- Finding
- Evidence
- Git commit
- Git file change
- Analysis run
- Architecture snapshot
- AI session

### Example TypeScript interfaces

```typescript
type SymbolKind =
  | "module"
  | "class"
  | "interface"
  | "function"
  | "method"
  | "variable"
  | "constant"
  | "type"
  | "enum"
  | "namespace"
  | "component"
  | "hook";

interface CodeSymbol {
  id: string;
  repositoryId: string;
  fileId: string;
  parentId?: string;
  name: string;
  qualifiedName: string;
  kind: SymbolKind;
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
  exported: boolean;
  visibility?: string;
  signature?: string;
  complexity?: number;
}

interface GraphEdge {
  id: string;
  sourceId: string;
  targetId: string;
  type: EdgeType;
  fileId?: string;
  line?: number;
  confidence: number;
  resolutionMethod?: string;
  evidence?: string;
}

interface Evidence {
  kind: "source" | "graph" | "metric" | "test" | "git";
  fileId?: string;
  path?: string;
  startLine?: number;
  endLine?: number;
  description: string;
  referenceId?: string;
}

interface Finding {
  id: string;
  repositoryId: string;
  analysisRunId: string;
  severity: "info" | "low" | "medium" | "high" | "critical";
  category:
    | "cycle"
    | "coupling"
    | "complexity"
    | "architecture"
    | "dead-code-candidate"
    | "change-risk"
    | "duplication";
  title: string;
  description: string;
  evidence: Evidence[];
  affectedNodeIds: string[];
  confidence: number;
  suggestions?: string[];
}
```

IDs should be stable across re-indexes where practical. Prefer deterministic IDs based on repository identity, normalized path, symbol qualified name, and source identity, while handling renames as a separate reconciliation problem.

## 8. Graph model

### Node types

- Repository
- Module
- File
- Symbol
- Package
- External dependency

### Edge types

- `CONTAINS`
- `IMPORTS`
- `EXPORTS`
- `CALLS`
- `EXTENDS`
- `IMPLEMENTS`
- `USES`
- `DEPENDS_ON`
- `TESTS`
- `GENERATES`

Edges should retain evidence and confidence. For example, selecting a `CALLS` edge should show the source expression and the exact file/line that supports it.

Do not assume every edge type is equally reliable. Imports are generally easier to resolve than runtime call relationships.

### Graph levels

```text
Repository
   ↓
Modules
   ↓
Files
   ↓
Symbols
```

Use progressive expansion. Never render all files and symbols at once.

## 9. Module and architecture inference

Do not assume directory boundaries are module boundaries.

Use several signals:

1. Explicit project boundaries: workspace manifests, package manifests, Nx configuration, Cargo workspaces, Go modules, Maven/Gradle projects, and .NET project files where supported.
2. Structural signals: directory layout and file relationships.
3. Dependency clusters: graph communities and dependency density.
4. Naming conventions: `features/`, `modules/`, `domain/`, `infrastructure/`, `controllers/`, and similar patterns.
5. User-defined overrides.

Allow users to rename, merge, split, or pin inferred modules.

### Architecture hypotheses

Potential classifications include:

- Layered architecture
- Clean/hexagonal architecture
- MVC/MVVM
- Modular monolith
- Microservices or multi-package system
- Feature-based architecture
- Repository pattern
- CQRS/event-driven patterns
- Plugin architecture

These must be **hypotheses with evidence**, not definitive labels inferred from directory names alone.

Example output:

```text
Architecture hypothesis: Modular monolith
Confidence: 0.81

Evidence:
- Several cohesive feature clusters
- Shared infrastructure layer
- Cross-feature dependencies are limited

Potential violations:
- Billing imports an internal Authentication implementation
- UI code imports a persistence adapter
```

Avoid false precision. Confidence should be calibrated against explicit rules and evaluation data; if it is only a heuristic score, label it as such.

## 10. Metrics and analysis

### Dependency metrics

- Fan-in: incoming dependency count.
- Fan-out: outgoing dependency count.
- Instability: `fanOut / (fanIn + fanOut)`, with a defined zero-denominator behavior.
- Abstractness: only where meaningful abstractions can be counted reliably.
- Distance from the main sequence: `abs(abstractness + instability - 1)`, only for suitable module-level analysis.

Do not apply class-oriented metrics indiscriminately to every language or module type.

### Circular dependencies

- Detect strongly connected components in the dependency graph.
- Present cycles with a concrete path and supporting import locations.
- Rank severity by cycle size, dependency scope, and architectural rules.
- Suggest potential break points as hypotheses; do not automatically edit source.

### Hotspots

Start with a transparent, configurable score combining normalized measures such as:

- Complexity
- Change frequency
- Churn
- Dependency centrality

Do not multiply raw metrics directly without normalization, because units and scales vary. Explain each score's components in the UI.

### Complexity

Start with language-specific cyclomatic complexity where practical. Record the calculation method and analyzer version. Add cognitive complexity only if a reliable implementation is available.

### Dead-code candidates

Possible signals:

- No statically discovered references
- Not reachable from known entry points
- Unused export
- Unused dependency

Always use “candidate” language. Dynamic imports, framework registration, reflection, plugins, dependency injection, and external consumers can make apparently unused code necessary.

### Duplicate code

Start with normalized syntax-tree fingerprints and configurable size thresholds. Keep textual similarity as a separate signal. Report matched ranges and avoid claiming semantic equivalence.

### Architecture rules

Allow user-defined dependency constraints. Example:

```yaml
rules:
  - name: domain-cannot-import-ui
    from: "src/domain/**"
    cannot_import:
      - "src/ui/**"

  - name: controllers-cannot-access-persistence
    from: "src/controllers/**"
    cannot_import:
      - "src/database/**"
      - "src/repositories/**"
```

Validate configuration, report rule matches with evidence, and support severity overrides.

## 11. Git archaeology

Use the installed Git executable initially, behind a Rust abstraction.

Extract:

- Commit metadata
- Changed files
- Insertions and deletions where available
- File change frequency
- Churn
- Co-change relationships
- Contributor history
- Rename information where available

### Change coupling

For files A and B, a Jaccard-style score can be:

`commits_changing_both(A, B) / commits_changing_either(A, B)`

Define the denominator as the union of relevant commit sets and handle empty sets. Exclude merge commits or treat them consistently according to configuration.

Show the observation window and sample size. High change coupling is evidence of historical co-change, not proof that the files form a logical module.

### Ownership

Display contributor proportions for contextual navigation only. Do not frame them as productivity or performance metrics. Handle email/name changes and anonymized Git history gracefully.

### Architecture timeline

Store snapshots of analysis results at selected commits or milestones. Support comparisons between snapshots, but avoid promising arbitrary historical reconstruction until the index can check out and analyze historical source safely and reproducibly.

### Change risk

Combine, transparently:

- Direct and transitive dependents
- Public entry points
- Test coverage/relationships where detectable
- Complexity
- Historical churn
- Co-change relationships
- Boundary crossings

Present the factors behind the risk score. Do not present it as a statistically calibrated failure probability unless validated against real outcomes.

## 12. Entry points and execution tracing

Identify likely entry points where supported:

- Application bootstrap and `main`
- HTTP route handlers
- CLI commands
- Message consumers
- Event handlers
- React routes
- Workers and scheduled jobs
- Tests

A trace is a graph-derived path through statically discovered relationships. Label uncertain edges and show unresolved calls.

Example:

```text
POST /users
  → UserController.create()
  → UserService.create()
  → UserRepository.insert()
  → Database adapter
```

Do not claim runtime certainty from static analysis alone. Dynamic execution tracing can be a future optional feature.

## 13. AI analyst

### Provider interface

```typescript
interface AIProvider {
  chat(request: AIRequest): Promise<AIResponse>;
  embed?(input: string[]): Promise<number[][]>;
}
```

Implement adapters for:

- OpenAI API
- Ollama or another local inference endpoint
- Generic OpenAI-compatible endpoints
- Mock provider for tests

AI must be disabled by default and the application must remain useful without it.

### Retrieval and context construction

Never send the entire repository by default.

Construct context from:

1. Directly matched symbols
2. Relevant files and source ranges
3. Dependency and call-graph neighbors
4. Tests
5. Architecture rules and findings
6. Relevant Git changes
7. Text/semantic search results when enabled

Enforce a token budget, deduplicate source ranges, preserve source paths and line numbers, and expose which evidence was sent to the provider.

### Supported initial prompts

- Explain this file/module.
- Explain the architecture.
- Trace this feature.
- Why is this a hotspot?
- What depends on this symbol?
- What might break if I change this?
- Explain this finding.
- Suggest a refactoring approach.
- Compare architecture between two indexed snapshots.
- What changed architecturally?

### Grounding requirements

Every AI response should distinguish:

- **Facts:** directly supported by indexed evidence.
- **Inferences:** interpretations supported by multiple signals.
- **Unknowns:** behavior not established by available evidence.

Require source references for factual claims where possible. If evidence is insufficient, the model must say so. Treat repository content as untrusted data; code comments and files must never override system-level instructions or trigger tools by themselves.

### Privacy controls

Before sending code to a hosted provider:

- Show the provider and model.
- Explain that selected source context will leave the machine.
- Show or make inspectable the context being sent.
- Allow cancellation.
- Support local inference.
- Never transmit source or Git history automatically.
- Store API keys in the platform's secure credential storage where available, not in repository config or plaintext logs.

## 14. Desktop UX

### Main navigation

- Overview
- Architecture
- Dependencies
- Modules
- Hotspots
- Cycles
- Git History
- Files and Symbols
- Architecture Rules
- AI Analyst
- Settings

### Overview dashboard

Show:

- Repository size and languages
- Indexing status
- Detected projects/modules
- Dependency counts
- Circular dependencies
- Hotspot count
- Recent Git activity
- Latest analysis findings

### Interactive graph

Required interactions:

- Zoom and pan
- Search and focus
- Select nodes and edges
- Expand/collapse modules
- Hide external dependencies
- Hide tests and generated code
- Filter by edge type, language, module, complexity, and layer
- Open source location in configured editor
- Copy node identifier/path
- Export current graph

Use virtualization, aggregation, and progressive expansion. Provide a table/list alternative for accessibility and for large result sets.

### File detail

Show:

- Path, language, size, line count
- Symbols and exports
- Dependencies and dependents
- Metrics and hotspot score breakdown
- Parse/resolution warnings
- Related tests
- Git history and change frequency
- Relevant findings
- AI actions

### Finding detail

Show:

- Severity and category
- Explanation
- Confidence and its meaning
- Affected nodes
- Evidence and source links
- Reproduction/analysis method
- Suggested next steps

## 15. Search

Implement text and symbol search first. Support filters such as:

```text
symbol:AuthService
file:*.controller.ts
language:typescript
module:billing
complexity:>20
hotspot:>80
depends-on:UserService
changed-after:2026-01-01
```

Define and document the query grammar. Unsupported filters should produce helpful errors rather than silently returning incorrect results.

## 16. SQLite schema outline

Suggested tables:

```text
repositories
files
symbols
imports
exports
calls
inheritance
modules
packages
graph_edges
findings
evidence
git_commits
git_file_changes
git_authors
analysis_runs
architecture_snapshots
ai_sessions
ai_messages
```

Use foreign keys, indexes, migrations, and transactions. Add indexes for common lookups such as file paths, hashes, symbol names, symbol-to-file relationships, edge endpoints/types, commit timestamps, and file-change relationships.

Store derived data with an analysis version so it can be invalidated and recomputed when algorithms change. Avoid persisting redundant metrics in multiple tables without a clear query/performance reason.

## 17. Configuration

Repository-level configuration:

```text
.archaeologist/
  config.yaml
  rules.yaml
```

Example:

```yaml
project:
  name: my-project

analysis:
  include:
    - "src/**"
  exclude:
    - "node_modules/**"
    - "dist/**"

architecture:
  module_patterns:
    - "src/features/**"
    - "src/core/**"
    - "src/infrastructure/**"

ai:
  enabled: false
  provider: ollama
  model: "<configured-model>"
```

Validate configuration with a schema, provide clear errors, and keep secrets out of repository files. Consider a user-level configuration for provider settings and a repository-level configuration for analysis rules.

## 18. CLI

The CLI and desktop app must use the same core library.

Example commands:

```bash
archaeologist scan ./project
archaeologist analyze ./project
archaeologist report ./project --format markdown
archaeologist graph ./project --format mermaid
archaeologist hotspots ./project
archaeologist cycles ./project
archaeologist explain ./project "authentication flow"
archaeologist diff ./project <snapshot-a> <snapshot-b>
```

CLI commands should support machine-readable JSON output, meaningful exit codes, cancellation, and structured logging. The AI-dependent command should fail clearly or offer a deterministic alternative when no provider is configured.

## 19. Export formats

Initial formats:

- Markdown report
- JSON model/findings
- CSV metrics
- Mermaid graph
- SVG/PNG graph export
- HTML report

Exported reports should include repository identity, analysis timestamp, tool/analyzer version, configuration summary, and known limitations.

## 20. Security and reliability

- Never execute repository code during scanning or static analysis.
- Do not automatically run package installation, tests, builds, or setup scripts.
- Treat filenames, comments, documentation, and source code as untrusted input.
- Avoid following symlinks outside the selected repository unless explicitly configured.
- Apply resource limits to parsing and graph analysis; handle pathological files gracefully.
- Do not log source contents or secrets by default.
- Support cancellation and safe recovery from interrupted indexing.
- Use atomic/transactional updates to avoid a partially updated graph.
- Keep AI tool execution disabled unless separately designed and explicitly authorized.
- If optional runtime analysis is introduced later, isolate it in a sandbox with timeouts, resource limits, restricted filesystem access, and networking disabled by default.

## 21. Testing strategy

### Unit tests

- Ignore-rule resolution
- File hashing and change detection
- Symbol extraction
- Import/export resolution
- Graph algorithms
- Cycle detection
- Metrics
- Hotspot normalization
- Change-coupling calculations
- Architecture-rule evaluation
- Query grammar
- Configuration validation

### Integration tests

- Index a small fixture repository
- Re-index after a file changes
- Handle a deleted or renamed file
- Resolve TypeScript path aliases
- Index a monorepo with multiple projects
- Import and export Git history
- Generate reports
- Recover after cancellation or interruption

### Golden fixtures

Maintain small representative repositories with expected symbols, edges, findings, and exported reports. Compare incremental results against clean full-index results.

### AI evaluation

- Use fixed question/evidence fixtures.
- Verify source citations and uncertainty handling.
- Include prompts attempting to make repository content override system instructions.
- Measure unsupported-claim rate and retrieval relevance.
- Keep AI tests deterministic where possible by using a mock provider.

## 22. Performance and observability

Measure:

- Files discovered and parsed per second
- Full-index duration
- Incremental-index duration
- Peak memory use
- SQLite database size
- Graph construction and analysis duration
- Search latency
- UI graph-render latency
- AI retrieval latency and token usage

Provide structured logs without source content. Establish performance baselines before making scale claims.

## 23. Implementation roadmap

### Phase 0 — Foundation

- Set up workspace and build pipeline.
- Create Tauri desktop shell and Rust core.
- Define domain types and SQLite migrations.
- Add structured logging, error types, and job/progress events.
- Build a small fixture repository for tests.

**Exit criteria:** application opens, selects a repository, persists settings, and runs a cancellable background job.

### Phase 1 — Archaeological core

- File scanner and ignore handling
- TypeScript/JavaScript detection
- Tree-sitter parsing
- Symbol/import/export extraction
- Basic local import resolution
- SQLite persistence
- Incremental indexing
- File/symbol search
- Basic dependency graph
- Cycle detection and initial metrics

**Exit criteria:** a user can index a real JS/TS repository, inspect its files/symbols/dependencies, and see evidence-backed cycle findings.

### Phase 2 — Usable graph and reports

- Graph expansion/collapse
- Filters and search-to-focus
- File/symbol detail views
- Finding detail and source navigation
- Markdown/JSON/Mermaid exports
- Robust cancellation and error recovery

**Exit criteria:** the core can be used without AI to investigate a repository and share a report.

### Phase 3 — Git archaeology

- Git CLI adapter
- Commit/file-change index
- Churn and change frequency
- Co-change relationships
- Contributor context
- Snapshot comparison
- Initial change-risk explanation

**Exit criteria:** users can identify historically coupled files and compare selected architecture snapshots.

### Phase 4 — AI analyst

- Provider abstraction
- OpenAI-compatible and Ollama adapters
- Context builder with token budget
- Evidence display and privacy confirmation
- Explain file/module/architecture
- Trace feature
- Explain hotspot/finding
- Change-impact analysis

**Exit criteria:** AI answers reference indexed evidence, distinguish facts from inference, and work with AI disabled or unavailable without breaking the app.

### Phase 5 — Architecture intelligence

- Architecture rules
- Better module inference
- Layer/boundary hypotheses
- Architecture comparison
- Structural duplication
- Dead-code candidates
- Improved hotspot and change-risk scoring

**Exit criteria:** findings are configurable, explainable, and validated against representative repositories.

### Phase 6 — Additional languages

- Python
- Rust
- Go
- Java
- C#

Prioritize languages based on actual user demand and the quality of available parser/resolution support.

### Phase 7 — Advanced features

- Historical architecture timeline
- Semantic retrieval, if justified by evaluation
- Optional runtime traces
- CI integration
- IDE/editor integration
- Plugin API for analyzers and exporters

## 24. First-release acceptance criteria

A release candidate should meet all of the following:

1. Open a local TypeScript/JavaScript repository without uploading its source.
2. Complete indexing in the background and report progress.
3. Re-index changed files and remove stale data after deletion/rename.
4. Show files, symbols, imports, exports, and resolved local dependencies.
5. Navigate from graph edges to source evidence.
6. Detect dependency cycles and explain the paths.
7. Show transparent complexity/coupling/hotspot metrics.
8. Search symbols and files with useful filters.
9. Export a Markdown and JSON report.
10. Cancel indexing without corrupting the index.
11. Work with AI completely disabled.
12. Never execute repository scripts automatically.
13. Include automated tests for full indexing, incremental indexing, graph analysis, and export.
14. Clearly label uncertain static-analysis conclusions.

## 25. Key product feature: change-impact analysis

The flagship workflow should be:

> **What would break if I changed this symbol?**

When a user selects a function, class, or module, collect:

1. Direct dependents
2. Transitive dependents
3. Known entry points reachable through those dependents
4. Related tests
5. Architecture-rule boundaries crossed
6. Historical co-change relationships
7. Complexity and churn indicators

Produce a transparent report:

```text
CHANGE IMPACT

Risk: High (heuristic)

Direct dependents: 17
Transitive dependents: 43
Known entry points: 6
Related tests: 12

Historical co-change:
- BillingService
- AuditService
- NotificationService

Potential concern:
This symbol is used across several feature boundaries.

Suggested next step:
Review whether a stable interface could replace the concrete dependency.
```

The score must be explained and must not be represented as a calibrated probability of failure.

## 26. Differentiating feature: architectural timeline

Allow users to compare architecture snapshots across selected commits or milestones.

Display changes in:

- Modules and project boundaries
- Dependency edges
- Cycles
- Hotspots
- Architecture-rule violations
- File churn and co-change relationships

The product should answer:

> “How did this codebase get to its current state?”

Use actual Git evidence and clearly identify gaps caused by missing history or unsupported language features.

## 27. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Static analysis misses dynamic behavior | Show resolution confidence and unknown relationships |
| AI invents relationships | Build context from deterministic evidence and require references |
| Large graphs become unusable | Aggregate, filter, virtualize, and expand progressively |
| Incremental indexing becomes inconsistent | Compare incremental output with clean-index golden fixtures |
| Hotspot scores mislead users | Normalize metrics and expose score components |
| Dead-code detection produces false positives | Report candidates and explain limitations |
| Multi-language support becomes too broad | Use adapters and release one language family at a time |
| Hosted AI leaks proprietary code | Explicit consent, inspectable context, local inference, AI disabled by default |
| Desktop and CLI logic diverge | Share the same Rust core |
| Scope expands indefinitely | Keep phases and exit criteria explicit |

## 28. Product positioning

**Short description**

> Codebase Archaeologist is a local-first software archaeology platform that reconstructs the architecture, dependencies, and evolution of existing codebases through static analysis, graph modeling, Git history, and AI-assisted reasoning.

**Portfolio description**

> Built a local-first desktop platform for software architecture discovery, combining syntax-aware indexing, dependency graphs, Git co-change analysis, architectural rules, and evidence-grounded AI explanations. Designed around an incremental Rust analysis engine shared by a React/Tauri desktop app and CLI.

## 29. Recommended implementation order

Do not start with the AI chat interface.

Start with the core data model and a small, testable end-to-end slice:

1. Open a repository.
2. Scan and hash files.
3. Parse TypeScript/JavaScript.
4. Extract symbols and imports.
5. Resolve local imports.
6. Persist files, symbols, and edges in SQLite.
7. Render a graph of one selected module.
8. Click an edge and open its source evidence.
9. Detect a dependency cycle in a fixture.
10. Re-index one changed file and prove that the graph updates correctly.

Once this works reliably, add Git history, findings, exports, and finally AI.

**The core product is the evidence-backed model of the codebase. The AI is a consumer of that model, not its source of truth.**
