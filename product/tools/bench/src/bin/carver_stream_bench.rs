use data_recovery_core::FileBlockSource;
use data_recovery_file_carver::{CarverConfig, carve_source_with_callbacks};
use std::fs::OpenOptions;
use std::ops::ControlFlow;
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path_arg = args
        .next()
        .ok_or("usage: carver-stream-bench <image> <size-bytes> <cancel-after-bytes>")?;
    let path = Path::new(&path_arg);
    let size_bytes: u64 = args.next().ok_or("missing size-bytes")?.parse()?;
    let cancel_after: u64 = args.next().ok_or("missing cancel-after-bytes")?.parse()?;

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.set_len(size_bytes)?;
    drop(file);

    let source = FileBlockSource::open(path)?;
    let started = Instant::now();
    let mut progress_events = 0_u64;
    let mut streamed_files = 0_u64;
    let mut scanned_bytes = 0_u64;
    let report = carve_source_with_callbacks(
        &source,
        &CarverConfig::default(),
        |progress| {
            progress_events += 1;
            scanned_bytes = progress.scanned_bytes;
            if progress.scanned_bytes >= cancel_after {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
        |_file| {
            streamed_files += 1;
            ControlFlow::Continue(())
        },
    )?;
    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "{{\"image_size_bytes\":{size_bytes},\"cancel_after_bytes\":{cancel_after},\"scanned_bytes\":{scanned_bytes},\"progress_events\":{progress_events},\"streamed_files\":{streamed_files},\"cancelled\":{},\"elapsed_seconds\":{elapsed:.6}}}",
        report.cancelled,
    );
    Ok(())
}
