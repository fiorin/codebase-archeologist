import { useEffect, useState } from "react";
import { api } from "../api";
import type { FileRecord, RepoSummary, SearchResult } from "../types";

interface Props {
  repo: RepoSummary;
  onOpenFile: (fileId: string, line?: number) => void;
  refreshKey: number;
}

export function Files({ repo, onOpenFile, refreshKey }: Props) {
  const [files, setFiles] = useState<FileRecord[]>([]);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[] | null>(null);
  const [limit, setLimit] = useState(200);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setLoading(true);
    api
      .listFiles(repo.repository_id, limit, 0)
      .then(setFiles)
      .finally(() => setLoading(false));
  }, [repo.repository_id, refreshKey, limit]);

  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults(null);
      return;
    }
    const t = setTimeout(() => {
      api.search(repo.repository_id, q, 100).then(setResults).catch(() => {});
    }, 200);
    return () => clearTimeout(t);
  }, [query, repo.repository_id, refreshKey]);

  return (
    <div className="flex h-full flex-col">
      <div className="mb-3 flex items-center gap-3">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search files and symbols…  (try: symbol:AuthService  or  file:.controller.ts)"
          className="flex-1 rounded border border-slate-700 bg-slate-900 px-3 py-2 text-sm outline-none focus:border-sky-600"
        />
        <span className="text-xs text-slate-500">
          {loading ? "loading…" : `${files.length} files`}
        </span>
      </div>

      <div className="min-h-0 flex-1 overflow-auto rounded border border-slate-800">
        {results ? (
          <table className="w-full text-left text-sm">
            <thead className="sticky top-0 bg-slate-900 text-xs uppercase text-slate-500">
              <tr>
                <th className="px-3 py-2">Result</th>
                <th className="px-3 py-2">Kind</th>
                <th className="px-3 py-2">Path</th>
                <th className="px-3 py-2">Line</th>
              </tr>
            </thead>
            <tbody>
              {results.map((r, i) => (
                <tr
                  key={`${r.file_id}-${i}`}
                  className="cursor-pointer border-t border-slate-800/70 hover:bg-slate-800/40"
                  onClick={() => onOpenFile(r.file_id, r.line ?? 1)}
                >
                  <td className="px-3 py-1.5 text-slate-200">
                    {r.symbol_name ?? r.path.split("/").pop()}
                  </td>
                  <td className="px-3 py-1.5 text-slate-400">
                    {r.kind ?? "file"}
                  </td>
                  <td className="max-w-lg truncate px-3 py-1.5 text-slate-400">
                    {r.path}
                  </td>
                  <td className="px-3 py-1.5 text-slate-500">
                    {r.line ?? ""}
                  </td>
                </tr>
              ))}
              {results.length === 0 && (
                <tr>
                  <td colSpan={4} className="px-3 py-6 text-center text-slate-500">
                    No matches.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        ) : (
          <table className="w-full text-left text-sm">
            <thead className="sticky top-0 bg-slate-900 text-xs uppercase text-slate-500">
              <tr>
                <th className="px-3 py-2">Path</th>
                <th className="px-3 py-2">Language</th>
                <th className="px-3 py-2">Lines</th>
                <th className="px-3 py-2">Status</th>
              </tr>
            </thead>
            <tbody>
              {files.map((f) => (
                <tr
                  key={f.id}
                  className="cursor-pointer border-t border-slate-800/70 hover:bg-slate-800/40"
                  onClick={() => onOpenFile(f.id, 1)}
                >
                  <td className="max-w-xl truncate px-3 py-1.5 text-slate-200">
                    {f.path}
                  </td>
                  <td className="px-3 py-1.5 text-slate-400">{f.language}</td>
                  <td className="px-3 py-1.5 text-slate-400">{f.line_count}</td>
                  <td className="px-3 py-1.5">
                    {f.parse_errors > 0 ? (
                      <span className="text-amber-400">
                        partial ({f.parse_errors})
                      </span>
                    ) : (
                      <span className="text-emerald-500">ok</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>

      {!results && files.length >= limit && (
        <div className="mt-2 text-center">
          <button
            onClick={() => setLimit((l) => l + 500)}
            className="rounded border border-slate-700 bg-slate-800 px-3 py-1 text-sm hover:bg-slate-700"
          >
            Load more
          </button>
        </div>
      )}
    </div>
  );
}
