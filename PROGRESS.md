# Codebase Archaeologist - Implementation Progress

## Constraints (invariants this project must keep)
- MVP: TypeScript/JavaScript only. AI is deferred to a later phase and OFF by default.
- Core analysis lives in Rust; UI is React+TS via Tauri. Rendering never does repo-wide analysis.
- Deterministic analysis first, evidence-backed conclusions, local-first.
- **Incremental indexing must converge to the same graph as a clean full index** (enforced by test).
- No repository code is ever executed during scanning/analysis.
- Every edge/finding links to source evidence (path + line + explanation).
- Metrics are transparent and explained; scores are heuristics, not calibrated probabilities.
- Cancelable jobs must leave the SQLite index in a consistent state.
- The app is fully usable with the AI layer absent.

## Status legend
- [x] done and verified   [~] partial   [ ] not started

## Vertical slice (plan lines 1098-1110) — COMPLETE
- [x] 1. Open a repository (folder picker → `open_repository`)
- [x] 2. Scan and hash files (`scanner.rs`, blake3, gitignore-aware, default excludes)
- [x] 3. Parse TypeScript/JavaScript (Tree-sitter, `lang/typescript.rs`)
- [x] 4. Extract symbols and imports (symbols, imports, exports, calls)
- [x] 5. Resolve local imports (relative + `tsconfig` paths + `baseUrl`, `resolve.rs`)
- [x] 6. Persist files, symbols, edges in SQLite (`db.rs`, `migrations/001_init.sql`)
- [x] 7. Render a graph of a selected module (`views/Graph.tsx`, React Flow, module + file levels)
- [x] 8. Click an edge and see its source evidence (edge panel + open-in-editor)
- [x] 9. Detect a dependency cycle in a fixture (Tarjan SCC, tested)
- [x] 10. Re-index one changed file and verify incremental == full (tested)
- [x] 11. Cancellation + DB consistency (tested)

## Phase 0 — Foundation — COMPLETE
- [x] Tauri 2 workspace (Rust + React 19 + TS + Vite + Tailwind v4)
- [x] Shared domain types (`core/types.rs`, `src/types.ts`)
- [x] SQLite schema + migrations + WAL + foreign keys
- [x] Progress events (`index-progress`, `index-completed`, `index-failed`)
- [x] Structured logging (tracing) + typed errors (`CoreError`)
- [x] Fixture test repo (integration tests build fixtures on the fly)

## Phase 1 — Archaeological core — COMPLETE (MVP subset)
- [x] File scanner + ignore rules (`.gitignore`, global/exclude, default dirs)
- [x] TS/JS (+TSX/JSX) detection
- [x] Tree-sitter TS/JS parsing with error tolerance (records parse errors)
- [x] Symbol/import/export extraction (+ best-effort calls, cyclomatic complexity)
- [x] Per-symbol cyclomatic complexity (not copied from file maximum)
- [x] Local import resolution (relative, tsconfig paths, baseUrl; external detection)
- [x] Incremental indexing with content hashing
- [x] File/symbol search (`symbol:`, `file:` prefixes)
- [x] Basic dependency graph (module + file levels, aggregated edges)
- [x] Cycle detection (Tarjan SCC) + metrics (fan-in/out, instability, hotspots)
- [x] Evidence-backed findings: cycles, high symbol complexity, high coupling,
  unresolved relative imports and parser diagnostics

## Phase 2 — Graph + Reports + UX — PARTIAL
- [x] Graph module/file levels, focus filter, progressive truncation
- [x] File/symbol detail view (symbols, deps, dependents, metrics, findings, source)
- [x] Finding detail + source navigation + open in editor
- [x] Change-impact view (direct/transitive dependents, heuristic risk)
- [x] Cancellation + error recovery
- [x] Golden invariant test (incremental == full)
- [ ] Markdown / JSON / Mermaid export
- [ ] Deeper graph interactions (collapse within graph, layer filters)

## Backlog (later phases)
- [~] Phase 3 — Git archaeology (90-day per-file churn via Git CLI is implemented;
  co-change and ownership remain pending)
- [ ] Phase 4 — AI analyst (provider abstraction, context builder, evidence-grounded, disabled by default)
- [ ] Phase 5 — Architecture rules, module inference, dead-code candidates, duplication
- [ ] Phase 6 — Additional languages (Python, Rust, Go, Java, C#)

## Verification checklist
- [x] Clean full index gives same graph as incremental on fixture (test `incremental_matches_full_index`)
- [x] No repository code executed
- [x] Uncertain edges labeled (confidence + resolution_method; dashed edges in UI)
- [x] Works offline / local-first
- [x] DB transactions on updates (per-file tx + analysis tx)
- [x] `cargo test` green (6/6 integration tests)
- [x] `npm run build` (tsc + vite) green

## How to run
- Dev: `npm run tauri dev`
- Tests: `cd src-tauri; cargo test`
- Frontend build: `npm run build`
- Headless scan (shared core, no UI): `cd src-tauri; cargo run --example scan -- <path>`

## Verification log
- `cargo test` → 6/6 integration tests pass (incl. incremental == full,
  symbol-level complexity, static findings and Git churn).
- Git churn is counted per file over the previous 90 days; if Git history is
  unavailable, the rest of indexing succeeds and churn contributes 0.
- `npx tsc --noEmit` and `npm run build` → clean.
- Real-world run: `cargo run --example scan -- ../src` on this project's own
  TS/TSX frontend → 11 files, 101 symbols, 22 resolved import edges,
  0 cycles, hotspot ranking produced (types.ts fan-in 8, App.tsx complexity 30)
  in ~55 ms.

## Key files
- Core: `src-tauri/src/core/{scanner,lang/typescript,resolve,graph,indexer,queries,db}.rs`
- Commands: `src-tauri/src/commands.rs`
- UI: `src/App.tsx`, `src/views/{Overview,Graph,Files,FileDetail,Findings,Hotspots}.tsx`
- Tests: `src-tauri/tests/indexing.rs`
