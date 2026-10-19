import { invoke } from "@tauri-apps/api/core";
import type {
  ChangeImpact,
  FileDetail,
  FileRecord,
  Finding,
  GraphView,
  IndexResult,
  MetricRecord,
  RepoSummary,
  SearchResult,
} from "./types";

export const api = {
  listRepositories: () => invoke<RepoSummary[]>("list_repositories"),

  openRepository: (path: string) =>
    invoke<RepoSummary>("open_repository", { path }),

  getSummary: (repositoryId: string) =>
    invoke<RepoSummary>("get_summary", { repositoryId }),

  startIndex: (repositoryId: string, root: string, mode: "full" | "incremental") =>
    invoke<{ job_id: number }>("start_index", { repositoryId, root, mode }),

  cancelIndex: (jobId: number) => invoke<void>("cancel_index", { jobId }),

  listFiles: (repositoryId: string, limit: number, offset: number) =>
    invoke<FileRecord[]>("list_files", { repositoryId, limit, offset }),

  getFileDetail: (repositoryId: string, fileId: string) =>
    invoke<FileDetail>("get_file_detail", { repositoryId, fileId }),

  getGraph: (repositoryId: string, level: "module" | "file", focus?: string) =>
    invoke<GraphView>("get_graph", { repositoryId, level, focus: focus ?? null }),

  getFindings: (repositoryId: string) =>
    invoke<Finding[]>("get_findings", { repositoryId }),

  getHotspots: (repositoryId: string, limit: number) =>
    invoke<MetricRecord[]>("get_hotspots", { repositoryId, limit }),

  search: (repositoryId: string, query: string, limit: number) =>
    invoke<SearchResult[]>("search_code", { repositoryId, query, limit }),

  getModules: (repositoryId: string) =>
    invoke<string[]>("get_modules", { repositoryId }),

  getChangeImpact: (repositoryId: string, fileId: string) =>
    invoke<ChangeImpact>("get_change_impact", { repositoryId, fileId }),

  readFileSnippet: (
    repositoryId: string,
    fileId: string,
    startLine: number,
    endLine: number,
  ) =>
    invoke<string>("read_file_snippet", {
      repositoryId,
      fileId,
      startLine,
      endLine,
    }),

  openInEditor: (path: string, line: number) =>
    invoke<void>("open_in_editor", { path, line }),
};

export type { IndexResult, RepoSummary };
