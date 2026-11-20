use data_recovery_core::{BlockSource, FileBlockSource, PhysicalDevice, ReadError, ReadResult};
use data_recovery_fs_ntfs::{read_boot_sector, read_mft_run_map};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

const BYTES_PER_SECTOR: u64 = 512;
const CLUSTER_SIZE: u64 = 4096;
const RECORD_SIZE: usize = 1024;
const BATCH_BYTES: usize = 4 * 1024 * 1024;

struct CountingSource {
    inner: FileBlockSource,
    read_calls: AtomicU64,
}

impl CountingSource {
    fn open(path: &Path) -> Result<Self, ReadError> {
        Ok(Self {
            inner: FileBlockSource::open(path)?,
            read_calls: AtomicU64::new(0),
        })
    }
}

impl BlockSource for CountingSource {
    fn device(&self) -> &PhysicalDevice {
        self.inner.device()
    }

    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<ReadResult, ReadError> {
        self.read_calls.fetch_add(1, Ordering::Relaxed);
        self.inner.read_at(offset, buffer)
    }
}

fn boot_sector(record_count: u64) -> [u8; 512] {
    let mut sector = [0_u8; 512];
    sector[0..3].copy_from_slice(&[0xeb, 0x52, 0x90]);
    sector[3..11].copy_from_slice(b"NTFS    ");
    sector[11..13].copy_from_slice(&(BYTES_PER_SECTOR as u16).to_le_bytes());
    sector[13] = 8;
    sector[21] = 0xf8;
    sector[24..26].copy_from_slice(&63_u16.to_le_bytes());
    sector[26..28].copy_from_slice(&255_u16.to_le_bytes());
    sector[28..32].copy_from_slice(&2048_u32.to_le_bytes());
    let mft_end = CLUSTER_SIZE + record_count * RECORD_SIZE as u64;
    let total_sectors = mft_end.div_ceil(BYTES_PER_SECTOR);
    sector[40..48].copy_from_slice(&total_sectors.to_le_bytes());
    sector[48..56].copy_from_slice(&1_u64.to_le_bytes());
    sector[56..64].copy_from_slice(&2_u64.to_le_bytes());
    sector[64] = (-10_i8) as u8;
    sector[68] = 1;
    sector[72..80].copy_from_slice(&0x0123_4567_89ab_cdef_u64.to_le_bytes());
    sector[510..512].copy_from_slice(&0xaa55_u16.to_le_bytes());
    sector
}

fn mft_record(index: u32) -> [u8; RECORD_SIZE] {
    let mut record = [0_u8; RECORD_SIZE];
    record[0..4].copy_from_slice(b"FILE");
    record[4..6].copy_from_slice(&48_u16.to_le_bytes());
    record[6..8].copy_from_slice(&3_u16.to_le_bytes());
    record[8..16].copy_from_slice(&0x1111_2222_3333_4444_u64.to_le_bytes());
    record[16..18].copy_from_slice(&7_u16.to_le_bytes());
    record[18..20].copy_from_slice(&2_u16.to_le_bytes());
    record[20..22].copy_from_slice(&56_u16.to_le_bytes());
    record[22..24].copy_from_slice(&1_u16.to_le_bytes());
    record[24..28].copy_from_slice(&384_u32.to_le_bytes());
    record[28..32].copy_from_slice(&(RECORD_SIZE as u32).to_le_bytes());
    record[40..42].copy_from_slice(&5_u16.to_le_bytes());
    record[44..48].copy_from_slice(&index.to_le_bytes());
    record[48..50].copy_from_slice(&0xabcd_u16.to_le_bytes());
    record[50..52].copy_from_slice(&0xaaaa_u16.to_le_bytes());
    record[52..54].copy_from_slice(&0xbbbb_u16.to_le_bytes());
    record[56..60].copy_from_slice(&0xffff_ffff_u32.to_le_bytes());
    record[510..512].copy_from_slice(&0xabcd_u16.to_le_bytes());
    record[1022..1024].copy_from_slice(&0xabcd_u16.to_le_bytes());
    record
}

fn generate_image(path: &Path, record_count: u64) -> Result<(), Box<dyn std::error::Error>> {
    let image_size = CLUSTER_SIZE + record_count * RECORD_SIZE as u64;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    file.set_len(image_size)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&boot_sector(record_count))?;
    file.seek(SeekFrom::Start(CLUSTER_SIZE))?;

    let records_per_chunk = BATCH_BYTES / RECORD_SIZE;
    let mut first_index = 0_u64;
    while first_index < record_count {
        let count = (record_count - first_index).min(records_per_chunk as u64) as usize;
        let mut chunk = Vec::with_capacity(count * RECORD_SIZE);
        for index in 0..count {
            chunk.extend_from_slice(&mft_record((first_index + index as u64) as u32));
        }
        file.write_all(&chunk)?;
        first_index += count as u64;
    }
    file.sync_all()?;
    Ok(())
}

fn checksum_record(record: &data_recovery_fs_ntfs::MftRecord) -> u64 {
    record.header.record_number.unwrap_or_default() as u64
}

fn run(path: &Path, mode: &str, record_count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let source = CountingSource::open(path)?;
    let boot = read_boot_sector(&source, 0)?;
    let map = read_mft_run_map(&source, &boot, 0)?;
    let started = Instant::now();
    let mut checksum = 0_u64;
    let mut parsed = 0_usize;

    match mode {
        "per-record" => {
            for index in 0..record_count as u64 {
                checksum = checksum
                    .wrapping_add(checksum_record(&map.read_record(&source, &boot, index)?));
                parsed += 1;
            }
        }
        "batched" => {
            let records_per_batch = (BATCH_BYTES / map.record_size()).max(1);
            let mut start = 0_usize;
            while start < record_count {
                let count = records_per_batch.min(record_count - start);
                for record in
                    map.read_records_batched(&source, &boot, start as u64, count, BATCH_BYTES)?
                {
                    checksum = checksum.wrapping_add(checksum_record(&record));
                    parsed += 1;
                }
                start += count;
            }
        }
        other => return Err(format!("unknown mode: {other}").into()),
    }

    let elapsed = started.elapsed().as_secs_f64();
    println!(
        "{{\"mode\":\"{mode}\",\"record_count\":{record_count},\"parsed\":{parsed},\"elapsed_seconds\":{elapsed:.6},\"read_calls\":{},\"checksum\":{checksum}}}",
        source.read_calls.load(Ordering::Relaxed)
    );
    Ok(())
}

fn usage() -> &'static str {
    "usage: ntfs-mft-bench generate <image> <record-count> | run <image> <per-record|batched> <record-count>"
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next().ok_or(usage())?;
    let path = args.next().ok_or(usage())?;
    let path = Path::new(&path);
    match command.as_str() {
        "generate" => {
            let record_count: u64 = args.next().ok_or(usage())?.parse()?;
            generate_image(path, record_count)?;
        }
        "run" => {
            let mode = args.next().ok_or(usage())?;
            let record_count: usize = args.next().ok_or(usage())?.parse()?;
            run(path, &mode, record_count)?;
        }
        _ => return Err(usage().into()),
    }
    Ok(())
}
