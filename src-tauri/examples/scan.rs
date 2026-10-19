use codebase_archaeologist_lib::core::db;
use codebase_archaeologist_lib::core::graph::DepGraph;
use codebase_archaeologist_lib::core::indexer::{IndexJob, Indexer};
use codebase_archaeologist_lib::core::queries;
use codebase_archaeologist_lib::core::types::ProgressEvent;
use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("usage: scan <path>"));
    let mut conn = db::open_memory().unwrap();
    let job = IndexJob::new();
    let mut noop = |_: ProgressEvent| {};
    let mut indexer = Indexer::new(&mut conn, root.clone(), &job, &mut noop);
    let repo_id = indexer.ensure_repository().unwrap();
    let result = indexer.index("full").unwrap();
    println!("{result:#?}");

    let summary = queries::summary(&conn, &repo_id).unwrap();
    println!("languages: {:#?}", summary.languages);

    let findings = queries::findings(&conn, &repo_id).unwrap();
    for f in &findings {
        println!("[{}/{}] {}", f.severity, f.category, f.title);
        for e in &f.evidence {
            println!(
                "    - {}:{} {}",
                e.path.as_deref().unwrap_or(""),
                e.start_line.unwrap_or(0),
                e.description
            );
        }
    }

    let hotspots = queries::hotspots(&conn, &repo_id, 5).unwrap();
    println!("top hotspots:");
    for h in hotspots {
        println!(
            "  {:.3}  {}  (fan-in {}, complexity {})",
            h.hotspot_score, h.path, h.fan_in, h.complexity
        );
    }

    let _ = DepGraph::default();
}
