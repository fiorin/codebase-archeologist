export interface RepoSummary {
  repository_id: string;
  root: string;
  name: string;
  file_count: number;
  symbol_count: number;
  edge_count: number;
  finding_count: number;
  languages: LanguageCount[];
  last_indexed: string | null;
}

export interface LanguageCount {
  language: string;
  count: number;
}

export interface FileRecord {
  id: string;
  repository_id: string;
  path: string;
  language: string;
  size: number;
  hash: string;
  line_count: number;
  parse_status: string;
  parse_errors: number;
}

export interface SymbolRecord {
  id: string;
  file_id: string;
  parent_id: string | null;
  name: string;
  qualified_name: string;
  kind: string;
  start_line: number;
  start_column: number;
  end_line: number;
  end_column: number;
  exported: boolean;
  signature: string | null;
  complexity: number | null;
}

export interface ImportSummary {
  path: string;
  file_id: string;
  specifier: string;
  kind: string;
  line: number;
  confidence: number;
  resolved: boolean;
  resolution_method: string | null;
}

export interface MetricRecord {
  file_id: string;
  path: string;
  fan_in: number;
  fan_out: number;
  instability: number;
  complexity: number;
  change_frequency: number;
  hotspot_score: number;
}

export interface Evidence {
  kind: string;
  file_id: string | null;
  path: string | null;
  start_line: number | null;
  end_line: number | null;
  description: string;
}

export interface Finding {
  id: string;
  severity: string;
  category: string;
  title: string;
  description: string;
  evidence: Evidence[];
  affected_node_ids: string[];
  confidence: number;
  suggestions: string[];
}

export interface FileDetail {
  file: FileRecord;
  symbols: SymbolRecord[];
  imports: ImportSummary[];
  dependents: ImportSummary[];
  metrics: MetricRecord | null;
  findings: Finding[];
}

export interface GraphNode {
  id: string;
  kind: string;
  label: string;
  file_id: string | null;
  module: string | null;
  data: Record<string, unknown>;
}

export interface GraphEdge {
  id: string;
  source: string;
  target: string;
  edge_type: string;
  confidence: number;
  resolution_method: string | null;
  evidence: Evidence;
}

export interface GraphView {
  nodes: GraphNode[];
  edges: GraphEdge[];
  truncated: boolean;
  level: string;
}

export interface SearchResult {
  file_id: string;
  path: string;
  symbol_name: string | null;
  qualified_name: string | null;
  kind: string | null;
  line: number | null;
}

export interface ProgressEvent {
  kind: string;
  message: string;
  current: number;
  total: number;
}

export interface IndexResult {
  run_id: string;
  repository_id: string;
  mode: string;
  files_scanned: number;
  files_parsed: number;
  files_skipped: number;
  symbols: number;
  edges: number;
  findings: number;
  cycles: number;
  duration_ms: number;
}

export interface ChangeImpact {
  file_id: string;
  path: string;
  direct_dependents: string[];
  transitive_dependents: string[];
  risk: string;
  factors: string[];
  explanation: string;
}
