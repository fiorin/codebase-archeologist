import { useEffect, useState } from "react";
import { api } from "../api";
import type { MetricRecord, RepoSummary } from "../types";

interface Props {
  repo: RepoSummary;
  onOpenFile: (fileId: string, line?: number) => void;
  refreshKey: number;
}

export function Hotspots({ repo, onOpenFile, refreshKey }: Props) {
  const [rows, setRows] = useState<MetricRecord[]>([]);

  useEffect(() => {
    api.getHotspots(repo.repository_id, 200).then(setRows).catch(() => {});
  }, [repo.repository_id, refreshKey]);

  return (
    <div className="flex h-full flex-col">
      <p className="mb-3 text-xs text-slate-500">
        Hotspot score = 0.35·normalized complexity + 0.30·normalized fan-in +
        0.20·normalized churn (Git changes in the last 90 days) +
        0.15·instability. Every component is normalized to 0..1, so scores are
        relative within this repository. Churn is 0 when Git history is
        unavailable or there were no recent changes.
      </p>
      <div className="min-h-0 flex-1 overflow-auto rounded border border-slate-800">
        <table className="w-full text-left text-sm">
          <thead className="sticky top-0 bg-slate-900 text-xs uppercase text-slate-500">
            <tr>
              <th className="px-3 py-2">File</th>
              <th className="px-3 py-2 text-right">Score</th>
              <th className="px-3 py-2 text-right">Complexity</th>
              <th className="px-3 py-2 text-right">Fan-in</th>
              <th className="px-3 py-2 text-right">Fan-out</th>
              <th className="px-3 py-2 text-right">Changes · 90d</th>
              <th className="px-3 py-2 text-right">Instability</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr
                key={r.file_id}
                className="cursor-pointer border-t border-slate-800/70 hover:bg-slate-800/40"
                onClick={() => onOpenFile(r.file_id, 1)}
              >
                <td className="max-w-xl truncate px-3 py-1.5 font-mono text-xs text-slate-200">
                  {r.path}
                </td>
                <td className="px-3 py-1.5 text-right text-sky-400">
                  {r.hotspot_score.toFixed(3)}
                </td>
                <td className="px-3 py-1.5 text-right text-slate-300">
                  {r.complexity}
                </td>
                <td className="px-3 py-1.5 text-right text-slate-300">
                  {r.fan_in}
                </td>
                <td className="px-3 py-1.5 text-right text-slate-300">
                  {r.fan_out}
                </td>
                <td className="px-3 py-1.5 text-right text-slate-300">
                  {r.change_frequency}
                </td>
                <td className="px-3 py-1.5 text-right text-slate-300">
                  {r.instability.toFixed(2)}
                </td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={7} className="px-3 py-6 text-center text-slate-500">
                  No metrics yet. Index a repository first.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
