import { useEffect, useState } from "react";
import { api } from "../api";
import type { Finding, RepoSummary } from "../types";

interface Props {
  repo: RepoSummary;
  onOpenFile: (fileId: string, line?: number) => void;
  refreshKey: number;
}

const severityColor: Record<string, string> = {
  critical: "border-red-800 bg-red-950/30 text-red-300",
  high: "border-orange-800 bg-orange-950/20 text-orange-300",
  medium: "border-amber-800 bg-amber-950/20 text-amber-300",
  low: "border-sky-800 bg-sky-950/20 text-sky-300",
  info: "border-slate-700 bg-slate-900/40 text-slate-300",
};

const categoryLabel: Record<string, string> = {
  cycle: "Circular dependency",
  coupling: "Coupling",
  complexity: "Complexity",
  unresolved_import: "Unresolved local import",
  parse_error: "Parser diagnostics",
  architecture: "Architecture",
  "dead-code-candidate": "Dead-code candidate",
  "change-risk": "Change risk",
  duplication: "Duplication",
};

export function Findings({ repo, onOpenFile, refreshKey }: Props) {
  const [findings, setFindings] = useState<Finding[]>([]);
  const [filter, setFilter] = useState<string>("all");

  useEffect(() => {
    api.getFindings(repo.repository_id).then(setFindings).catch(() => {});
  }, [repo.repository_id, refreshKey]);

  const categories = Array.from(new Set(findings.map((f) => f.category)));
  const shown =
    filter === "all" ? findings : findings.filter((f) => f.category === filter);

  return (
    <div className="flex h-full flex-col">
      <div className="mb-3 flex items-center gap-2">
        <select
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          className="rounded border border-slate-700 bg-slate-900 px-2 py-1 text-sm"
        >
          <option value="all">All categories</option>
          {categories.map((c) => (
            <option key={c} value={c}>
              {categoryLabel[c] ?? c}
            </option>
          ))}
        </select>
        <span className="text-xs text-slate-500">
          {shown.length} findings — all conclusions are evidence-backed
          hypotheses, not facts
        </span>
      </div>

      <div className="min-h-0 flex-1 space-y-3 overflow-auto pr-1">
        {shown.map((f) => (
          <article
            key={f.id}
            className={`rounded-lg border p-4 ${
              severityColor[f.severity] ?? severityColor.info
            }`}
          >
            <div className="flex items-center justify-between">
              <h3 className="font-semibold">{f.title}</h3>
              <div className="flex items-center gap-2 text-xs opacity-80">
                <span className="uppercase">{f.severity}</span>
                <span>·</span>
                <span>{categoryLabel[f.category] ?? f.category}</span>
                <span>·</span>
                <span>confidence {(f.confidence * 100).toFixed(0)}%</span>
              </div>
            </div>
            <pre className="mt-2 whitespace-pre-wrap font-sans text-sm text-slate-300">
              {f.description}
            </pre>

            {f.evidence.length > 0 && (
              <div className="mt-3">
                <div className="text-xs uppercase text-slate-500">Evidence</div>
                <ul className="mt-1 space-y-1">
                  {f.evidence.slice(0, 8).map((ev, i) => (
                    <li key={i} className="text-xs text-slate-400">
                      {ev.file_id && (
                        <button
                          onClick={() =>
                            onOpenFile(ev.file_id!, ev.start_line ?? 1)
                          }
                          className="mr-2 rounded bg-slate-800 px-1.5 py-0.5 text-sky-300 hover:bg-slate-700"
                        >
                          open
                        </button>
                      )}
                      <span className="font-mono">{ev.path}</span>
                      {ev.start_line ? `:${ev.start_line}` : ""} —{" "}
                      {ev.description}
                    </li>
                  ))}
                </ul>
              </div>
            )}

            {f.suggestions.length > 0 && (
              <div className="mt-3 text-xs text-slate-400">
                <span className="uppercase text-slate-500">Next steps: </span>
                {f.suggestions.join(" ")}
              </div>
            )}
          </article>
        ))}
        {shown.length === 0 && (
          <p className="text-sm text-slate-500">
            No findings. Run an analysis to generate them.
          </p>
        )}
      </div>
    </div>
  );
}
