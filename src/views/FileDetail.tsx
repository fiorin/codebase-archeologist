import { useEffect, useState } from "react";
import { api } from "../api";
import type { ChangeImpact, FileDetail, RepoSummary } from "../types";

interface Props {
  repo: RepoSummary;
  fileId: string;
  initialLine?: number;
  onOpenFile: (fileId: string, line?: number) => void;
  onClose: () => void;
  refreshKey: number;
}

const severityColor: Record<string, string> = {
  critical: "text-red-400",
  high: "text-orange-400",
  medium: "text-amber-300",
  low: "text-sky-300",
  info: "text-slate-400",
};

export function FileDetailView({
  repo,
  fileId,
  initialLine,
  onOpenFile,
  onClose,
  refreshKey,
}: Props) {
  const [detail, setDetail] = useState<FileDetail | null>(null);
  const [impact, setImpact] = useState<ChangeImpact | null>(null);
  const [snippet, setSnippet] = useState<string>("");
  const [error, setError] = useState<string>("");

  useEffect(() => {
    setDetail(null);
    setImpact(null);
    setSnippet("");
    setError("");
    api
      .getFileDetail(repo.repository_id, fileId)
      .then(async (d) => {
        setDetail(d);
        const line = initialLine ?? 1;
        const snip = await api
          .readFileSnippet(repo.repository_id, fileId, Math.max(1, line - 5), line + 30)
          .catch(() => "");
        setSnippet(snip);
      })
      .catch((e) => setError(String(e)));
    api.getChangeImpact(repo.repository_id, fileId).then(setImpact).catch(() => {});
  }, [repo.repository_id, fileId, initialLine, refreshKey]);

  if (error) {
    return (
      <div className="p-6 text-sm text-red-400">
        Failed to load file detail: {error}
      </div>
    );
  }

  if (!detail) {
    return <div className="p-6 text-sm text-slate-500">Loading…</div>;
  }

  const fullPath = `${repo.root}/${detail.file.path}`;

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <div className="flex items-start justify-between border-b border-slate-800 p-4">
        <div className="min-w-0">
          <div className="truncate font-mono text-sm text-slate-200">
            {detail.file.path}
          </div>
          <div className="mt-1 flex gap-3 text-xs text-slate-500">
            <span>{detail.file.language}</span>
            <span>{detail.file.line_count} lines</span>
            <span>{detail.file.size} bytes</span>
            <span className="font-mono">{detail.file.hash.slice(0, 12)}</span>
            {detail.file.parse_errors > 0 && (
              <span className="text-amber-400">
                {detail.file.parse_errors} parse issues
              </span>
            )}
          </div>
        </div>
        <div className="flex shrink-0 gap-2">
          <button
            onClick={() => api.openInEditor(fullPath, initialLine ?? 1)}
            className="rounded border border-slate-700 bg-slate-800 px-3 py-1 text-xs hover:bg-slate-700"
          >
            Open in editor
          </button>
          <button
            onClick={onClose}
            className="rounded border border-slate-700 px-3 py-1 text-xs text-slate-400 hover:bg-slate-800"
          >
            Close
          </button>
        </div>
      </div>

      <div className="grid min-h-0 flex-1 grid-cols-3 divide-x divide-slate-800 overflow-hidden">
        {/* Left: symbols */}
        <div className="overflow-auto p-4">
          <h3 className="mb-2 text-xs font-semibold uppercase text-slate-500">
            Symbols ({detail.symbols.length})
          </h3>
          <ul className="space-y-1">
            {detail.symbols.map((s) => (
              <li
                key={s.id}
                className="cursor-pointer rounded px-2 py-1 text-sm hover:bg-slate-800/50"
                onClick={() => api.readFileSnippet(repo.repository_id, fileId, s.start_line, s.end_line).then(setSnippet)}
              >
                <div className="flex items-center gap-2">
                  <span className="text-[10px] uppercase text-slate-500">
                    {s.kind}
                  </span>
                  {s.exported && (
                    <span className="rounded bg-emerald-900/40 px-1 text-[10px] text-emerald-400">
                      export
                    </span>
                  )}
                  <span className="truncate font-mono text-slate-200">
                    {s.qualified_name}
                  </span>
                </div>
                <div className="text-xs text-slate-500">
                  line {s.start_line}
                  {s.complexity != null && s.complexity > 1 && (
                    <span title="Estimated cyclomatic complexity for this symbol">
                      {" "}· cyclomatic complexity {s.complexity}
                    </span>
                  )}
                </div>
                {s.signature && (
                  <div className="truncate font-mono text-[11px] text-slate-600">
                    {s.signature}
                  </div>
                )}
              </li>
            ))}
          </ul>
        </div>

        {/* Middle: dependencies + metrics + impact */}
        <div className="space-y-4 overflow-auto p-4">
          <section>
            <h3 className="mb-2 text-xs font-semibold uppercase text-slate-500">
              Depends on ({detail.imports.length})
            </h3>
            <ul className="space-y-1 text-sm">
              {detail.imports.map((i, idx) => (
                <li
                  key={idx}
                  className="cursor-pointer truncate font-mono text-xs text-sky-300 hover:underline"
                  onClick={() => onOpenFile(i.file_id, i.line)}
                  title={`via "${i.specifier}" (${i.resolution_method}, ${(i.confidence * 100).toFixed(0)}%)`}
                >
                  {i.path}
                </li>
              ))}
            </ul>
          </section>

          <section>
            <h3 className="mb-2 text-xs font-semibold uppercase text-slate-500">
              Depended on by ({detail.dependents.length})
            </h3>
            <ul className="space-y-1 text-sm">
              {detail.dependents.map((i, idx) => (
                <li
                  key={idx}
                  className="cursor-pointer truncate font-mono text-xs text-violet-300 hover:underline"
                  onClick={() => onOpenFile(i.file_id, i.line)}
                >
                  {i.path}
                </li>
              ))}
            </ul>
          </section>

          {detail.metrics && (
            <section>
              <h3 className="mb-2 text-xs font-semibold uppercase text-slate-500">
                Metrics
              </h3>
              <div className="grid grid-cols-2 gap-2 text-xs">
                <div className="rounded bg-slate-900/60 p-2">
                  Fan-in: {detail.metrics.fan_in}
                </div>
                <div className="rounded bg-slate-900/60 p-2">
                  Fan-out: {detail.metrics.fan_out}
                </div>
                <div className="rounded bg-slate-900/60 p-2">
                  Instability: {detail.metrics.instability.toFixed(2)}
                </div>
                <div className="rounded bg-slate-900/60 p-2">
                  Complexity: {detail.metrics.complexity}
                </div>
                <div className="col-span-2 rounded bg-slate-900/60 p-2">
                  Hotspot score:{" "}
                  <span className="text-sky-400">
                    {detail.metrics.hotspot_score.toFixed(3)}
                  </span>
                  <div className="mt-1 text-[10px] text-slate-500">
                    weighted mix of normalized complexity, fan-in, Git changes
                    in the last 90 days and instability
                  </div>
                </div>
              </div>
            </section>
          )}

          {impact && (
            <section>
              <h3 className="mb-2 text-xs font-semibold uppercase text-slate-500">
                Change impact
              </h3>
              <div className="rounded border border-slate-800 bg-slate-900/40 p-2 text-xs">
                <div className="mb-1">
                  Risk:{" "}
                  <span className="font-semibold text-amber-300">
                    {impact.risk} (heuristic)
                  </span>
                </div>
                <div className="text-slate-400">
                  {impact.direct_dependents.length} direct ·{" "}
                  {impact.transitive_dependents.length} transitive dependents
                </div>
                <div className="mt-1 text-[10px] text-slate-500">
                  {impact.explanation}
                </div>
              </div>
            </section>
          )}

          {detail.findings.length > 0 && (
            <section>
              <h3 className="mb-2 text-xs font-semibold uppercase text-slate-500">
                Findings
              </h3>
              <ul className="space-y-1 text-xs">
                {detail.findings.map((f) => (
                  <li key={f.id}>
                    <span className={severityColor[f.severity] ?? ""}>
                      [{f.severity}]
                    </span>{" "}
                    {f.title}
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>

        {/* Right: source */}
        <div className="overflow-auto bg-slate-950 p-4">
          <h3 className="mb-2 text-xs font-semibold uppercase text-slate-500">
            Source
          </h3>
          <pre className="whitespace-pre font-mono text-[11px] leading-relaxed text-slate-300">
            {snippet || "—"}
          </pre>
        </div>
      </div>
    </div>
  );
}
