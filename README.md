<div align="center">

# Codebase Archaeologist

### Explore the architecture behind your code.

A local-first desktop tool for mapping dependencies, uncovering architectural
risks, and understanding how a codebase fits together.

![License](https://img.shields.io/badge/license-not--selected-lightgrey.svg)
![Status](https://img.shields.io/badge/status-MVP-blue.svg)

</div>

---

## Overview

Codebase Archaeologist analyzes a local JavaScript or TypeScript repository and
builds an evidence-backed model of its structure and dependencies. Explore
relationships between modules and files, inspect evidence attached to graph
edges, find circular dependencies, and identify complexity and coupling
hotspots—all from a desktop application.

Analysis and indexing run locally. Your repository is not uploaded, and the
application does not execute repository code. AI-assisted analysis is not part
of the current MVP.

## Why Codebase Archaeologist?

Understanding an unfamiliar codebase often means jumping between files, tracing
imports manually, and piecing together architecture from conventions and partial
documentation. Codebase Archaeologist turns those relationships into a
navigable model and links its findings to evidence in the source.

- **Explore relationships:** navigate module- and file-level dependency graphs.
- **Follow the evidence:** inspect import evidence and open files related to
  graph edges and findings.
- **Find architectural friction:** detect dependency cycles, high-complexity
  symbols, unresolved local imports, parser diagnostics, and highly coupled
  files.
- **Assess change impact:** inspect direct and transitive dependents with a
  heuristic risk estimate.
- **Keep analysis local:** indexing and persistence happen on your machine.

## Current capabilities

- Open and index a local JavaScript or TypeScript repository, including TSX and
  JSX files.
- Run indexing in the background with progress reporting and cancellation.
- Parse source with Tree-sitter and extract functions, classes, interfaces,
  types, enums, namespaces, components, hooks, imports, and exports.
- Extract best-effort call candidates and estimate per-symbol cyclomatic
  complexity.
- Resolve local imports, including relative paths and `tsconfig.json` /
  `jsconfig.json` `paths` aliases and `baseUrl`.
- Persist the index in SQLite and update it incrementally using content hashes.
- Explore interactive module- and file-level dependency graphs powered by React
  Flow.
- Detect circular dependencies using Tarjan's strongly connected components
  algorithm, with representative paths and supporting import evidence.
- Surface evidence-backed findings for unresolved relative imports, parser
  diagnostics, high-complexity symbols, and highly coupled files.
- Inspect fan-in, fan-out, instability, and a transparent hotspot score.
- Include per-file Git churn from the last 90 days in hotspot analysis when
  local Git history is available; otherwise, churn contributes zero.
- Inspect change impact through direct and transitive dependents and a heuristic
  risk estimate.
- Search files and symbols using `symbol:` and `file:` filters.

## Screenshots

Screenshots and a short demo will be added here as the UI stabilizes.

<!-- Suggested assets:
![Architecture graph](docs/images/architecture-graph.png)
![Circular dependency finding](docs/images/circular-dependency.png)
![Change impact analysis](docs/images/change-impact.png)
-->

## Installation

There are no published installers yet. To run the application from source, you
will need:

- Node.js and npm
- Rust and Cargo
- The [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your
  operating system

From the repository root:

```bash
npm install
npm run tauri dev
```

Build the frontend:

```bash
npm run build
```

Run the Rust test suite from `src-tauri`:

```bash
cd src-tauri
cargo test
```

## Quick start

1. Launch Codebase Archaeologist with `npm run tauri dev`.
2. Open a local repository containing JavaScript or TypeScript source.
3. Wait for indexing to finish; progress is shown in the application.
4. Explore the module- or file-level dependency graph.
5. Select nodes and edges to inspect relationships and supporting evidence.
6. Review cycles, findings, hotspots, and change-impact information.
7. Search for a symbol with `symbol:` or a path with `file:`.

## How it works

```text
Local repository
      |
      v
File scanner and content hashing
      |
      v
Tree-sitter source analysis
      |
      v
Symbols, imports, exports, and call candidates
      |
      v
Local import resolution
      |
      v
SQLite index and incremental updates
      |
      v
Dependency graph and static analysis
      |
      v
Interactive exploration and findings
```

## Technology stack

| Area | Technology |
| --- | --- |
| Desktop | Tauri 2 |
| Frontend | React, TypeScript, React Flow |
| Core engine | Rust |
| Parsing | Tree-sitter |
| Local storage | SQLite |
| Change history | Git, when available |

## Architecture

The frontend communicates with the Rust core through Tauri commands and events.
Repository scanning, parsing, indexing, graph analysis, and query operations
are handled by the core rather than by the browser UI.

```text
React + TypeScript UI (src/)
            |
            | Tauri commands / events
            v
Rust core (src-tauri/src/core/)
  scanner -> Tree-sitter analyzers -> import resolver
          -> SQLite indexer -> graph analysis -> queries
```

Key implementation modules:

| Module | Responsibility |
| --- | --- |
| `src-tauri/src/core/scanner.rs` | File discovery, ignore handling, and hashing |
| `src-tauri/src/core/lang/typescript.rs` | Tree-sitter extraction for TypeScript/JavaScript |
| `src-tauri/src/core/resolve.rs` | Local import resolution |
| `src-tauri/src/core/graph.rs` | Strongly connected components, metrics, hotspots, and change impact |
| `src-tauri/src/core/indexer.rs` | Indexing pipeline, Git churn, and incremental updates |
| `src-tauri/src/core/queries.rs` | Read model consumed by the UI |
| `src-tauri/src/commands.rs` | Tauri command interface |

## Analysis limitations

- Static analysis describes relationships inferred from source code; it is not
  a complete representation of runtime behavior.
- JavaScript and TypeScript are the only supported languages in the current
  MVP.
- Call candidates are best-effort and may not resolve dynamic dispatch,
  reflection, dependency injection, runtime registration, or other dynamic
  behavior.
- A missing reference does not prove that code is unused.
- Change-impact risk and hotspot scores are heuristics, not calibrated
  probabilities of defects. The hotspot score is relative to the indexed
  repository.
- Git-based churn depends on local Git history and covers the most recent 90
  days.
- AI-assisted reasoning is not currently included.
- Findings are investigation aids. Review the source evidence before making
  changes.

## Privacy and security

- Repository content is processed locally and is not uploaded by the MVP.
- Repository code is not executed during analysis.
- AI integration is not included in the current MVP.

For security concerns, open a private vulnerability report through the hosting
platform if the repository enables that feature. A dedicated `SECURITY.md` and
coordinated disclosure process are not yet in place.

## Roadmap

Potential next areas include:

- Deeper Git history, including co-change analysis and ownership signals
- Architecture rules and boundary validation
- Architecture snapshot comparison
- Markdown, JSON, and Mermaid exports
- Additional language analyzers
- Optional, evidence-grounded AI explanations

The roadmap is directional, not a guarantee. Check the issue tracker and
milestones for current priorities.

## Contributing

Bug reports, focused feature proposals, and pull requests are welcome. Before
opening a pull request:

1. Search existing issues to avoid duplicates.
2. Describe the problem and proposed solution.
3. Add or update tests for behavior changes.
4. Run the relevant checks locally (`npm run build`, `cargo test`, and
   `cargo fmt --check`).
5. Keep changes focused and document heuristic limitations.

A formal `CONTRIBUTING.md` with development conventions and review guidelines
will be added as the project prepares for broader community contributions.

## License

No license has been selected yet, and there is currently no root-level
`LICENSE` file. Add a license and its complete text before publishing this
project as open source; until then, do not assume that the code is available
under an open-source license. Update the badge at the top once a license is
chosen.

## Acknowledgements

Codebase Archaeologist is built with open-source tools including Tauri, React,
React Flow, Rust, Tree-sitter, and SQLite. See the dependency manifests for the
complete dependency list and consult each dependency's license terms.
