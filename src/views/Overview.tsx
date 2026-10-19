import { useEffect, useState } from "react";
import { api } from "../api";
import type { RepoSummary, Finding, MetricRecord } from "../types";

interface Props {
  repo: RepoSummary;
  onOpenFile: (fileId: string, line?: number) => void;
  refreshKey: number;
}

function Stat({ label, value }: { label: string; value: string | number }) {
  return (
    <div className="rounded-lg border border-slate-800 bg-slate-900/60 p-4">
      <div className="text-2xl font-semibold text-slate-100">{value}</div>
      <div className="mt-1 text-xs uppercase tracking-wide text-slate-500">
        {label}
      </div>
    </div>
  );
}

export function Overview({ repo, onOpenFile, refreshKey }: Props) {
  const [findings, setFindings] = useState<Finding[]>([]);
  const [hotspots, setHotspots] = useState<MetricRecord[]>([]);

  useEffect(() => {
    api.getFindings(repo.repository_id).then(setFindings).catch(() => {});
    api.getHotspots(repo.repository_id, 8).then(setHotspots).catch(() => {});
  }, [repo.repository_id, refreshKey]);

  const cycles = findings.filter((f) => f.category === "cycle");

  return (
    <div className="space-y-6">
      <div className="grid grid-cols-4 gap-4">
        <Stat label="Files" value={repo.file_count} />
        <Stat label="Symbols" value={repo.symbol_count} />
        <Stat label="Dependency edges" value={repo.edge_count} />
        <Stat label="Findings" value={repo.finding_count} />
      </div>

      <div className="grid grid-cols-2 gap-6">
        <section className="rounded-lg border border-slate-800 bg-slate-900/40 p-4">
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
            Languages
          </h2>
          {repo.languages.length === 0 && (
            <p className="text-sm text-slate-500">No files indexed yet.</p>
          )}
          <ul className="space-y-2">
            {repo.languages.map((l) => (
              <li key={l.language} className="flex items-center gap-3 text-sm">
                <span className="w-24 text-slate-300">{l.language}</span>
                <div className="h-2 flex-1 overflow-hidden rounded bg-slate-800">
                  <div
                    className="h-full bg-sky-500"
                    style={{
                      width: `${Math.min(
                        100,
                        (l.count / Math.max(1, repo.file_count)) * 100,
                      )}%`,
                    }}
                  />
                </div>
                <span className="w-10 text-right text-slate-400">{l.count}</span>
              </li>
            ))}
          </ul>
        </section>

        <section className="rounded-lg border border-slate-800 bg-slate-900/40 p-4">
          <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
            Circular dependencies
          </h2>
          {cycles.length === 0 ? (
            <p className="text-sm text-slate-500">
              No cycles detected in the resolved dependency graph.
            </p>
          ) : (
            <ul className="space-y-2">
              {cycles.slice(0, 5).map((c) => (
                <li
                  key={c.id}
                  className="rounded border border-amber-900/50 bg-amber-950/20 p-2 text-sm"
                >
                  <div className="font-medium text-amber-300">{c.title}</div>
                  <div className="mt-1 text-xs text-slate-400">
                    confidence {(c.confidence * 100).toFixed(0)}%
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      </div>

      <section className="rounded-lg border border-slate-800 bg-slate-900/40 p-4">
        <h2 className="mb-3 text-sm font-semibold uppercase tracking-wide text-slate-400">
          Top hotspots
        </h2>
        {hotspots.length === 0 ? (
          <p className="text-sm text-slate-500">No metrics computed yet.</p>
        ) : (
          <table className="w-full text-left text-sm">
            <thead className="text-xs uppercase text-slate-500">
              <tr>
                <th className="py-1 pr-4">File</th>
                <th className="py-1 pr-4">Score</th>
                <th className="py-1 pr-4">Fan-in</th>
                <th className="py-1 pr-4">Fan-out</th>
                <th className="py-1 pr-4">Complexity</th>
              </tr>
            </thead>
            <tbody>
              {hotspots.map((h) => (
                <tr
                  key={h.file_id}
                  className="cursor-pointer border-t border-slate-800 hover:bg-slate-800/40"
                  onClick={() => onOpenFile(h.file_id)}
                >
                  <td className="max-w-md truncate py-1 pr-4 text-slate-200">
                    {h.path}
                  </td>
                  <td className="py-1 pr-4 text-sky-400">
                    {h.hotspot_score.toFixed(3)}
                  </td>
                  <td className="py-1 pr-4 text-slate-300">{h.fan_in}</td>
                  <td className="py-1 pr-4 text-slate-300">{h.fan_out}</td>
                  <td className="py-1 pr-4 text-slate-300">{h.complexity}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
