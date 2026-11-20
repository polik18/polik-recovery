use data_recovery_core::FileBlockSource;
use data_recovery_file_carver::{CarverConfig, carve_source};
use std::collections::BTreeMap;
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .ok_or("usage: polik-recovery-carver-bench <image>")?;
    let path = PathBuf::from(path);
    let source = FileBlockSource::open(&path)?;
    let carved = carve_source(&source, &CarverConfig::default())?;
    let mut by_type = BTreeMap::<String, usize>::new();
    for item in &carved {
        *by_type.entry(item.file_type.clone()).or_default() += 1;
    }

    let type_counts = by_type
        .into_iter()
        .map(|(file_type, count)| format!("\"{file_type}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"carved_count\":{},\"by_type\":{{{type_counts}}}}}",
        carved.len()
    );
    Ok(())
}
