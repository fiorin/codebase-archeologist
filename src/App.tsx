import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import "./App.css";
import { api } from "./api";
import type {
  IndexResult,
  ProgressEvent,
  RepoSummary,
} from "./types";
import { Overview } from "./views/Overview";
import { Graph } from "./views/Graph";
import { Files } from "./views/Files";
import { FileDetailView } from "./views/FileDetail";
import { Findings } from "./views/Findings";
import { Hotspots } from "./views/Hotspots";

type Tab = "overview" | "graph" | "cycles" | "files" | "hotspots";

const TABS: { id: Tab; label: string }[] = [
  { id: "overview", label: "Overview" },
  { id: "graph", label: "Dependencies" },
  { id: "cycles", label: "Findings" },
  { id: "files", label: "Files & Symbols" },
  { id: "hotspots", label: "Hotspots" },
];

export default function App() {
  const [repos, setRepos] = useState<RepoSummary[]>([]);
  const [repo, setRepo] = useState<RepoSummary | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [detail, setDetail] = useState<{ fileId: string; line?: number } | null>(
    null,
  );
  const [progress, setProgress] = useState<ProgressEvent | null>(null);
  const [jobId, setJobId] = useState<number | null>(null);
  const [lastResult, setLastResult] = useState<IndexResult | null>(null);
  const [error, setError] = useState<string>("");
  const [refreshKey, setRefreshKey] = useState(0);

  const refreshRepo = useCallback(async (id: string) => {
    try {
      const summary = await api.getSummary(id);
      setRepo(summary);
      setRepos((prev) => {
        const others = prev.filter((r) => r.repository_id !== id);
        return [...others, summary].sort((a, b) => a.name.localeCompare(b.name));
      });
    } catch {
      /* ignore */
    }
  }, []);

  useEffect(() => {
    api.listRepositories().then((r) => {
      setRepos(r);
      if (r.length > 0) setRepo(r[0]);
    });
  }, []);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    listen<ProgressEvent>("index-progress", (e) => {
      setProgress(e.payload);
    }).then((u) => unlisteners.push(u));

    listen<IndexResult>("index-completed", (e) => {
      setProgress(null);
      setJobId(null);
      setLastResult(e.payload);
      refreshRepo(e.payload.repository_id);
      setRefreshKey((k) => k + 1);
    }).then((u) => unlisteners.push(u));

    listen<{ message: string; cancelled: boolean }>("index-failed", (e) => {
      setProgress(null);
      setJobId(null);
      setError(
        e.payload.cancelled
          ? "Indexing cancelled."
          : `Indexing failed: ${e.payload.message}`,
      );
    }).then((u) => unlisteners.push(u));

    return () => unlisteners.forEach((u) => u());
  }, [refreshRepo]);

  const pickRepository = async () => {
    setError("");
    const selected = await open({ directory: true, multiple: false });
    if (!selected || Array.isArray(selected)) return;
    try {
      const summary = await api.openRepository(selected);
      setRepo(summary);
      setRepos((prev) => {
        const others = prev.filter(
          (r) => r.repository_id !== summary.repository_id,
        );
        return [...others, summary].sort((a, b) => a.name.localeCompare(b.name));
      });
      setTab("overview");
    } catch (e) {
      setError(String(e));
    }
  };

  const runIndex = async (mode: "full" | "incremental") => {
    if (!repo) return;
    setError("");
    setLastResult(null);
    try {
      const { job_id } = await api.startIndex(
        repo.repository_id,
        repo.root,
        mode,
      );
      setJobId(job_id);
      setProgress({ kind: "index.started", message: "Starting…", current: 0, total: 0 });
    } catch (e) {
      setError(String(e));
    }
  };

  const cancelIndex = async () => {
    if (jobId != null) {
      await api.cancelIndex(jobId);
    }
  };

  const indexing = jobId != null;

  const pct =
    progress && progress.total > 0
      ? Math.floor((progress.current / progress.total) * 100)
      : 0;

  return (
    <div className="flex h-full flex-col">
      {/* Top bar */}
      <header className="flex items-center gap-3 border-b border-slate-800 bg-slate-950 px-4 py-2">
        <div className="flex items-center gap-2">
          <span className="text-lg">🏛️</span>
          <span className="font-semibold text-slate-100">
            Codebase Archaeologist
          </span>
        </div>

        <div className="mx-2 h-5 w-px bg-slate-800" />

        {repos.length > 0 && (
          <select
            value={repo?.repository_id ?? ""}
            onChange={(e) => {
              const r = repos.find((x) => x.repository_id === e.target.value);
              if (r) {
                setRepo(r);
                setDetail(null);
              }
            }}
            className="max-w-xs rounded border border-slate-700 bg-slate-900 px-2 py-1 text-sm"
          >
            {repos.map((r) => (
              <option key={r.repository_id} value={r.repository_id}>
                {r.name}
              </option>
            ))}
          </select>
        )}

        <button
          onClick={pickRepository}
          className="rounded border border-slate-700 bg-slate-800 px-3 py-1 text-sm hover:bg-slate-700"
        >
          Open repository…
        </button>

        <div className="ml-auto flex items-center gap-2">
          {indexing ? (
            <button
              onClick={cancelIndex}
              className="rounded border border-red-800 bg-red-950/40 px-3 py-1 text-sm text-red-300 hover:bg-red-950/70"
            >
              Cancel
            </button>
          ) : (
            <>
              <button
                disabled={!repo}
                onClick={() => runIndex("full")}
                className="rounded border border-sky-800 bg-sky-950/40 px-3 py-1 text-sm text-sky-300 hover:bg-sky-950/70 disabled:opacity-40"
              >
                Full index
              </button>
              <button
                disabled={!repo}
                onClick={() => runIndex("incremental")}
                className="rounded border border-slate-700 bg-slate-800 px-3 py-1 text-sm hover:bg-slate-700 disabled:opacity-40"
              >
                Incremental
              </button>
            </>
          )}
        </div>
      </header>

      {/* Progress bar */}
      {indexing && (
        <div className="border-b border-slate-800 bg-slate-900/60 px-4 py-1.5">
          <div className="flex items-center gap-3 text-xs text-slate-400">
            <span className="w-40 shrink-0">{progress?.kind ?? "working"}</span>
            <div className="h-1.5 flex-1 overflow-hidden rounded bg-slate-800">
              <div
                className="h-full bg-sky-500 transition-all"
                style={{ width: `${pct}%` }}
              />
            </div>
            <span className="w-32 shrink-0 truncate text-right">
              {progress?.message}
            </span>
          </div>
        </div>
      )}

      {error && (
        <div className="border-b border-red-900 bg-red-950/40 px-4 py-1.5 text-xs text-red-300">
          {error}
        </div>
      )}

      {lastResult && !indexing && (
        <div className="border-b border-slate-800 bg-emerald-950/20 px-4 py-1.5 text-xs text-emerald-300">
          {lastResult.mode} index: {lastResult.files_parsed} parsed,{" "}
          {lastResult.files_skipped} unchanged, {lastResult.symbols} symbols,{" "}
          {lastResult.edges} edges, {lastResult.cycles} cycles in{" "}
          {lastResult.duration_ms} ms
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {/* Sidebar */}
        <nav className="w-44 shrink-0 border-r border-slate-800 bg-slate-950 p-2">
          {TABS.map((t) => (
            <button
              key={t.id}
              onClick={() => {
                setTab(t.id);
                setDetail(null);
              }}
              className={`mb-1 w-full rounded px-3 py-2 text-left text-sm ${
                tab === t.id
                  ? "bg-sky-950/50 text-sky-300"
                  : "text-slate-400 hover:bg-slate-800/60"
              }`}
            >
              {t.label}
            </button>
          ))}
        </nav>

        {/* Main */}
        <main className="min-h-0 min-w-0 flex-1 overflow-hidden">
          {!repo ? (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-slate-500">
              <p>No repository open.</p>
              <button
                onClick={pickRepository}
                className="rounded border border-sky-800 bg-sky-950/40 px-4 py-2 text-sky-300 hover:bg-sky-950/70"
              >
                Open a TypeScript/JavaScript repository
              </button>
              <p className="max-w-md text-center text-xs">
                Analysis runs entirely on your machine. Nothing is uploaded.
                Repository scripts are never executed.
              </p>
            </div>
          ) : repo.file_count === 0 ? (
            <div className="flex h-full flex-col items-center justify-center gap-3 text-slate-500">
              <p>{repo.root}</p>
              <button
                onClick={() => runIndex("full")}
                className="rounded border border-sky-800 bg-sky-950/40 px-4 py-2 text-sky-300 hover:bg-sky-950/70"
              >
                Run full index
              </button>
            </div>
          ) : detail ? (
            <FileDetailView
              repo={repo}
              fileId={detail.fileId}
              initialLine={detail.line}
              refreshKey={refreshKey}
              onOpenFile={(fileId, line) => setDetail({ fileId, line })}
              onClose={() => setDetail(null)}
            />
          ) : tab === "graph" ? (
            <Graph
              repo={repo}
              refreshKey={refreshKey}
              onOpenFile={(fileId, line) => setDetail({ fileId, line })}
            />
          ) : (
            <div className="h-full p-4">
              {tab === "overview" && (
                <Overview
                  repo={repo}
                  refreshKey={refreshKey}
                  onOpenFile={(fileId, line) => setDetail({ fileId, line })}
                />
              )}
              {tab === "cycles" && (
                <Findings
                  repo={repo}
                  refreshKey={refreshKey}
                  onOpenFile={(fileId, line) => setDetail({ fileId, line })}
                />
              )}
              {tab === "files" && (
                <Files
                  repo={repo}
                  refreshKey={refreshKey}
                  onOpenFile={(fileId, line) => setDetail({ fileId, line })}
                />
              )}
              {tab === "hotspots" && (
                <Hotspots
                  repo={repo}
                  refreshKey={refreshKey}
                  onOpenFile={(fileId, line) => setDetail({ fileId, line })}
                />
              )}
            </div>
          )}
        </main>
      </div>
    </div>
  );
}
