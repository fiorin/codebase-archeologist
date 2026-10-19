import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  type Edge,
  type Node,
  type NodeMouseHandler,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { api } from "../api";
import type { GraphEdge, RepoSummary } from "../types";

interface Props {
  repo: RepoSummary;
  onOpenFile: (fileId: string, line?: number) => void;
  refreshKey: number;
}

const SEVERITY_COLORS: Record<string, string> = {
  module: "#0ea5e9",
  file: "#334155",
};

export function Graph({ repo, onOpenFile, refreshKey }: Props) {
  const [level, setLevel] = useState<"module" | "file">("module");
  const [modules, setModules] = useState<string[]>([]);
  const [focus, setFocus] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [edges, setEdges] = useState<Edge[]>([]);
  const [selectedEdge, setSelectedEdge] = useState<GraphEdge | null>(null);
  const [truncated, setTruncated] = useState(false);

  useEffect(() => {
    api.getModules(repo.repository_id).then(setModules).catch(() => {});
  }, [repo.repository_id, refreshKey]);

  const load = useCallback(async () => {
    setLoading(true);
    try {
      const view = await api.getGraph(
        repo.repository_id,
        level,
        level === "file" && focus ? focus : undefined,
      );

      // Simple circular-ish layout.
      const count = Math.max(1, view.nodes.length);
      const radius = Math.max(220, count * 22);
      const layout: Node[] = view.nodes.map((n, i) => {
        const angle = (i / count) * Math.PI * 2;
        return {
          id: n.id,
          position: {
            x: 600 + radius * Math.cos(angle),
            y: 400 + radius * Math.sin(angle),
          },
          data: {
            label: n.module ?? n.label.split("/").pop() ?? n.label,
            full: n.label,
            kind: n.kind,
            fileId: n.file_id,
            fileCount: (n.data as { fileCount?: number }).fileCount,
          },
          style: {
            background: SEVERITY_COLORS[n.kind] ?? "#334155",
            color: "#e2e8f0",
            border: "1px solid #1e293b",
            borderRadius: 8,
            padding: 6,
            fontSize: 11,
            width: 180,
          },
        };
      });

      const flowEdges: Edge[] = view.edges.map((e) => ({
        id: e.id,
        source: e.source,
        target: e.target,
        animated: false,
        style: {
          stroke: e.confidence >= 0.9 ? "#475569" : "#a16207",
          strokeWidth: level === "module" ? 2 : 1,
          strokeDasharray: e.confidence >= 0.9 ? undefined : "4 3",
        },
        data: { edge: e },
      }));

      setNodes(layout);
      setEdges(flowEdges);
      setTruncated(view.truncated);
    } finally {
      setLoading(false);
    }
  }, [repo.repository_id, level, focus]);

  useEffect(() => {
    load();
  }, [load]);

  const onNodeDoubleClick: NodeMouseHandler = useCallback(
    (_evt, node) => {
      const fileId = (node.data as { fileId?: string }).fileId;
      if (fileId) {
        onOpenFile(fileId);
      } else if ((node.data as { kind?: string }).kind === "module") {
        setLevel("file");
        setFocus((node.data as { full: string }).full);
      }
    },
    [onOpenFile],
  );

  const onEdgeClick = useCallback(
    (_evt: React.MouseEvent, edge: Edge) => {
      setSelectedEdge((edge.data as { edge: GraphEdge }).edge);
    },
    [],
  );

  const legend = useMemo(
    () => (
      <div className="pointer-events-none absolute bottom-3 left-3 z-10 rounded border border-slate-800 bg-slate-950/80 p-2 text-xs text-slate-400">
        <div>Solid = relative import (high confidence)</div>
        <div>Dashed = alias / lower confidence</div>
        <div>Double-click module → open files</div>
      </div>
    ),
    [],
  );

  return (
    <div className="relative h-full">
      <div className="absolute left-3 top-3 z-10 flex items-center gap-2">
        <select
          value={level}
          onChange={(e) => {
            const v = e.target.value as "module" | "file";
            setLevel(v);
            if (v === "module") setFocus("");
          }}
          className="rounded border border-slate-700 bg-slate-900 px-2 py-1 text-sm"
        >
          <option value="module">Module level</option>
          <option value="file">File level</option>
        </select>
        {level === "file" && (
          <select
            value={focus}
            onChange={(e) => setFocus(e.target.value)}
            className="max-w-xs rounded border border-slate-700 bg-slate-900 px-2 py-1 text-sm"
          >
            <option value="">All modules</option>
            {modules.map((m) => (
              <option key={m} value={m}>
                {m}
              </option>
            ))}
          </select>
        )}
        <button
          onClick={load}
          className="rounded border border-slate-700 bg-slate-800 px-3 py-1 text-sm hover:bg-slate-700"
        >
          Refresh
        </button>
        {loading && <span className="text-xs text-slate-400">loading…</span>}
        {truncated && (
          <span className="text-xs text-amber-400">
            graph truncated for performance — narrow the focus
          </span>
        )}
      </div>

      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodeDoubleClick={onNodeDoubleClick}
        onEdgeClick={onEdgeClick}
        fitView
        minZoom={0.05}
      >
        <Background color="#1e293b" gap={24} />
        <Controls />
        <MiniMap
          nodeColor={(n) =>
            ((n.style?.background as string) ?? "#334155") as string
          }
          style={{ background: "#0b0e14" }}
        />
      </ReactFlow>

      {legend}

      {selectedEdge && (
        <div className="absolute bottom-3 right-3 z-10 w-96 rounded border border-slate-700 bg-slate-950/95 p-3 text-sm">
          <div className="flex items-center justify-between">
            <span className="font-semibold text-slate-200">Edge evidence</span>
            <button
              onClick={() => setSelectedEdge(null)}
              className="text-slate-500 hover:text-slate-300"
            >
              ✕
            </button>
          </div>
          <div className="mt-2 space-y-1 text-xs text-slate-400">
            <div>
              <span className="text-slate-500">Type:</span>{" "}
              {selectedEdge.edge_type}
            </div>
            <div>
              <span className="text-slate-500">Confidence:</span>{" "}
              {(selectedEdge.confidence * 100).toFixed(0)}%
            </div>
            <div>
              <span className="text-slate-500">Resolved by:</span>{" "}
              {selectedEdge.resolution_method ?? "n/a"}
            </div>
            <div className="pt-1 text-slate-300">
              {selectedEdge.evidence.description}
            </div>
            {selectedEdge.evidence.path && (
              <button
                onClick={() =>
                  selectedEdge.evidence.file_id &&
                  onOpenFile(
                    selectedEdge.evidence.file_id,
                    selectedEdge.evidence.start_line ?? 1,
                  )
                }
                className="mt-2 rounded bg-sky-900/40 px-2 py-1 text-sky-300 hover:bg-sky-900/70"
              >
                Open source: {selectedEdge.evidence.path}
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
