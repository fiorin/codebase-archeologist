use crate::core::types::{Evidence, Finding, GraphEdge, MetricRecord};
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Serialize)]
pub struct Cycle {
    pub id: String,
    pub nodes: Vec<String>,
    pub paths: Vec<CyclePath>,
    pub size: usize,
    pub severity: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CyclePath {
    pub from: String,
    pub to: String,
    pub via: Vec<String>,
    pub evidence: Vec<Evidence>,
}

/// Adjacency of file-level dependency edges (file_id -> file_id).
#[derive(Debug, Default)]
pub struct DepGraph {
    pub out_edges: HashMap<String, Vec<GraphEdge>>,
    pub in_edges: HashMap<String, Vec<GraphEdge>>,
}

impl DepGraph {
    pub fn build(edges: &[GraphEdge]) -> Self {
        let mut g = DepGraph::default();
        for e in edges {
            g.out_edges
                .entry(e.source.clone())
                .or_default()
                .push(e.clone());
            g.in_edges
                .entry(e.target.clone())
                .or_default()
                .push(e.clone());
        }
        g
    }

    pub fn fan_in(&self, id: &str) -> u32 {
        self.in_edges.get(id).map(|v| v.len() as u32).unwrap_or(0)
    }

    pub fn fan_out(&self, id: &str) -> u32 {
        self.out_edges.get(id).map(|v| v.len() as u32).unwrap_or(0)
    }
}

/// Tarjan's strongly connected components, iterative to avoid stack overflow.
pub fn strongly_connected_components(g: &DepGraph) -> Vec<Vec<String>> {
    let mut nodes: Vec<String> = g.out_edges.keys().cloned().collect();
    let inbound_only: Vec<String> = g
        .in_edges
        .keys()
        .filter(|k| !g.out_edges.contains_key(*k))
        .cloned()
        .collect();
    nodes.extend(inbound_only);
    nodes.sort();
    nodes.dedup();

    let index_of: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_str(), i))
        .collect();
    let n = nodes.len();

    let mut index = vec![usize::MAX; n];
    let mut lowlink = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut counter = 0usize;
    let mut sccs: Vec<Vec<String>> = Vec::new();

    for start in 0..n {
        if index[start] != usize::MAX {
            continue;
        }
        // (node, next_child_idx)
        let mut work: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some((v, ci)) = work.pop() {
            if ci == 0 {
                index[v] = counter;
                lowlink[v] = counter;
                counter += 1;
                stack.push(v);
                on_stack[v] = true;
            }

            let neighbors: Vec<usize> = g
                .out_edges
                .get(&nodes[v])
                .map(|edges| {
                    edges
                        .iter()
                        .filter_map(|e| index_of.get(e.target.as_str()).copied())
                        .collect()
                })
                .unwrap_or_default();

            let mut advanced = false;
            for (k, &w) in neighbors.iter().enumerate().skip(ci) {
                if index[w] == usize::MAX {
                    work.push((v, k + 1));
                    work.push((w, 0));
                    advanced = true;
                    break;
                } else if on_stack[w] {
                    lowlink[v] = lowlink[v].min(index[w]);
                }
            }
            if advanced {
                continue;
            }

            if lowlink[v] == index[v] {
                let mut comp = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack[w] = false;
                    comp.push(nodes[w].clone());
                    if w == v {
                        break;
                    }
                }
                comp.sort();
                sccs.push(comp);
            }

            if let Some(&(parent, _)) = work.last() {
                lowlink[parent] = lowlink[parent].min(lowlink[v]);
            }
        }
    }

    sccs.retain(|c| c.len() > 1 || self_loop(g, &c[0]));
    sccs.sort_by_key(|c| std::cmp::Reverse(c.len()));
    sccs
}

fn self_loop(g: &DepGraph, node: &str) -> bool {
    g.out_edges
        .get(node)
        .map(|e| e.iter().any(|x| x.target == node))
        .unwrap_or(false)
}

pub fn find_cycles(g: &DepGraph, label: impl Fn(&str) -> String) -> Vec<Cycle> {
    let sccs = strongly_connected_components(g);
    let mut cycles = Vec::new();
    for (i, comp) in sccs.iter().enumerate() {
        let evidence: Vec<Evidence> = comp
            .iter()
            .flat_map(|n| {
                g.out_edges
                    .get(n)
                    .into_iter()
                    .flatten()
                    .filter(|e| comp.contains(&e.target))
                    .map(|e| e.evidence.clone())
            })
            .collect();

        // Find a representative concrete path through the SCC.
        let mut paths = shortest_cycle_paths(g, comp);
        if paths.is_empty() && !evidence.is_empty() {
            paths.push(CyclePath {
                from: comp[0].clone(),
                to: comp[0].clone(),
                via: comp.to_vec(),
                evidence,
            });
        }

        let severity = match comp.len() {
            2..=3 => "medium",
            4..=7 => "high",
            _ => "critical",
        }
        .to_string();

        let labels: Vec<String> = comp.iter().map(|n| label(n)).collect();
        cycles.push(Cycle {
            id: format!("cycle-{i}"),
            nodes: labels,
            paths,
            size: comp.len(),
            severity,
            confidence: 0.95,
        });
    }
    cycles
}

fn shortest_cycle_paths(g: &DepGraph, comp: &[String]) -> Vec<CyclePath> {
    let set: HashSet<&str> = comp.iter().map(|s| s.as_str()).collect();
    let mut out = Vec::new();

    for start in comp.iter().take(6) {
        let Some(edges) = g.out_edges.get(start) else {
            continue;
        };
        for edge in edges {
            if !set.contains(edge.target.as_str()) {
                continue;
            }
            if let Some(via) = bfs_within(g, &edge.target, start, &set) {
                out.push(CyclePath {
                    from: start.clone(),
                    to: edge.target.clone(),
                    via,
                    evidence: vec![edge.evidence.clone()],
                });
            }
            if out.len() >= 4 {
                return out;
            }
        }
    }
    out
}

fn bfs_within(g: &DepGraph, from: &str, goal: &str, set: &HashSet<&str>) -> Option<Vec<String>> {
    if from == goal {
        return Some(vec![]);
    }
    let mut queue = VecDeque::new();
    queue.push_back((from.to_string(), vec![from.to_string()]));
    let mut visited: HashSet<String> = HashSet::new();
    visited.insert(from.to_string());

    while let Some((node, path)) = queue.pop_front() {
        if path.len() > set.len() + 1 {
            continue;
        }
        let Some(edges) = g.out_edges.get(&node) else {
            continue;
        };
        for e in edges {
            if !set.contains(e.target.as_str()) {
                continue;
            }
            if e.target == goal {
                let mut p = path.clone();
                p.push(e.target.clone());
                return Some(p);
            }
            if visited.insert(e.target.clone()) {
                let mut p = path.clone();
                p.push(e.target.clone());
                queue.push_back((e.target.clone(), p));
            }
        }
    }
    None
}

/// Hotspot score: normalized, explainable combination of complexity,
/// fan-in/centrality, churn and instability. Each component is 0..1.
#[derive(Debug, Clone, Serialize)]
pub struct HotspotBreakdown {
    pub file_id: String,
    pub path: String,
    pub complexity: u32,
    pub fan_in: u32,
    pub fan_out: u32,
    pub churn: u32,
    pub components: HashMap<String, f64>,
    pub score: f64,
    pub explanation: String,
}

pub fn compute_metrics(
    edges: &DepGraph,
    complexities: &HashMap<String, u32>,
    churn: &HashMap<String, u32>,
    paths: &HashMap<String, String>,
) -> (Vec<MetricRecord>, Vec<HotspotBreakdown>) {
    let ids: HashSet<&str> = paths.keys().map(|s| s.as_str()).collect();

    let max_complexity = complexities.values().copied().max().unwrap_or(1).max(1);
    let max_fan_in = ids
        .iter()
        .map(|id| edges.fan_in(id))
        .max()
        .unwrap_or(1)
        .max(1);
    let max_churn = churn.values().copied().max().unwrap_or(1).max(1);

    let mut records = Vec::new();
    let mut hotspots = Vec::new();

    let mut sorted_ids: Vec<&str> = ids.iter().copied().collect();
    sorted_ids.sort();

    for id in sorted_ids {
        let fan_in = edges.fan_in(id);
        let fan_out = edges.fan_out(id);
        let instability = if fan_in + fan_out == 0 {
            0.0
        } else {
            fan_out as f64 / (fan_in + fan_out) as f64
        };
        let complexity = complexities.get(id).copied().unwrap_or(0);
        let churn = churn.get(id).copied().unwrap_or(0);

        let c_complexity = complexity as f64 / max_complexity as f64;
        let c_centrality = fan_in as f64 / max_fan_in as f64;
        let c_instability = instability;
        let c_churn = churn as f64 / max_churn as f64;

        // Transparent, documented weights.
        let score =
            0.35 * c_complexity + 0.30 * c_centrality + 0.20 * c_churn + 0.15 * c_instability;
        let score = (score * 1000.0).round() / 1000.0;

        let path = paths.get(id).cloned().unwrap_or_default();

        let mut components = HashMap::new();
        components.insert("complexity".into(), c_complexity);
        components.insert("centrality".into(), c_centrality);
        components.insert("churn".into(), c_churn);
        components.insert("instability".into(), c_instability);

        hotspots.push(HotspotBreakdown {
            file_id: id.to_string(),
            path: path.clone(),
            complexity,
            fan_in,
            fan_out,
            churn,
            components,
            score,
            explanation: format!(
                "score = 0.35*complexity({c_complexity:.2}) + 0.30*fan-in({c_centrality:.2}) + 0.20*churn({c_churn:.2}) + 0.15*instability({c_instability:.2}) = {score:.3}"
            ),
        });

        records.push(MetricRecord {
            file_id: id.to_string(),
            path,
            fan_in,
            fan_out,
            instability,
            complexity,
            change_frequency: churn,
            hotspot_score: score,
        });
    }

    (records, hotspots)
}

pub fn cycle_findings(cycles: &[Cycle], label: impl Fn(&str) -> String) -> Vec<Finding> {
    cycles
        .iter()
        .map(|c| {
            let path_desc = c
                .nodes
                .iter()
                .map(|n| label(n))
                .collect::<Vec<_>>()
                .join(" → ");
            let confidence = c.confidence;
            Finding {
                id: c.id.clone(),
                severity: c.severity.clone(),
                category: "cycle".into(),
                title: format!("Circular dependency among {} files", c.size),
                description: format!(
                    "A strongly connected component of {} files forms a dependency cycle:\n{path_desc}\n\nDetected with Tarjan's algorithm over resolved import edges (confidence {confidence}).",
                    c.size
                ),
                evidence: c
                    .paths
                    .iter()
                    .flat_map(|p| p.evidence.clone())
                    .collect(),
                affected_node_ids: c.nodes.clone(),
                confidence: c.confidence,
                suggestions: vec![
                    "Break the cycle by extracting a shared interface or moving shared types into a lower-level module.".into(),
                    "Consider inverting a dependency so one direction disappears.".into(),
                ],
            }
        })
        .collect()
}

/// Reachability-based change impact for a single file.
#[derive(Debug, Clone, Serialize)]
pub struct ChangeImpact {
    pub file_id: String,
    pub path: String,
    pub direct_dependents: Vec<String>,
    pub transitive_dependents: Vec<String>,
    pub risk: String,
    pub factors: Vec<String>,
    pub explanation: String,
}

pub fn change_impact(g: &DepGraph, file_id: &str, label: impl Fn(&str) -> String) -> ChangeImpact {
    let mut visited: HashSet<String> = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(file_id.to_string());

    let mut direct = Vec::new();
    let mut depth = 0usize;

    while !queue.is_empty() {
        let level_size = queue.len();
        for _ in 0..level_size {
            let node = queue.pop_front().unwrap();
            let Some(edges) = g.in_edges.get(&node) else {
                continue;
            };
            for e in edges {
                if !visited.insert(e.source.clone()) {
                    continue;
                }
                if depth == 0 {
                    direct.push(e.source.clone());
                }
                queue.push_back(e.source.clone());
            }
        }
        depth += 1;
        if depth > 12 {
            break;
        }
    }

    let transitive: Vec<String> = visited.iter().cloned().collect();
    let risk = match transitive.len() {
        0..=2 => "low",
        3..=10 => "medium",
        11..=40 => "high",
        _ => "critical",
    }
    .to_string();

    ChangeImpact {
        file_id: file_id.to_string(),
        path: label(file_id),
        direct_dependents: direct.iter().map(|d| label(d)).collect(),
        transitive_dependents: transitive.iter().map(|d| label(d)).collect(),
        risk,
        factors: vec![
            format!("{} direct dependents", direct.len()),
            format!("{} transitive dependents", transitive.len()),
        ],
        explanation: format!(
            "Heuristic risk based on {} transitive dependents. This is not a calibrated failure probability.",
            transitive.len()
        ),
    }
}
