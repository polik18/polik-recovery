use crate::block::{BlockSource, ByteRange, ReadError};
use crate::hashing::Sha256;

pub const DEFAULT_CHUNK_SIZE: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub offset: u64,
    pub data: Vec<u8>,
}

impl Chunk {
    pub fn size_bytes(&self) -> usize {
        self.data.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadSummary {
    pub range: ByteRange,
    pub chunk_size: usize,
    pub bytes_read: u64,
    pub chunks_read: u64,
    pub sha256: String,
}

pub struct ChunkedReader<'a, S: BlockSource + ?Sized> {
    source: &'a S,
    range: ByteRange,
    chunk_size: usize,
    cursor: u64,
}

impl<'a, S: BlockSource + ?Sized> ChunkedReader<'a, S> {
    pub fn new(source: &'a S, range: ByteRange, chunk_size: usize) -> Result<Self, ReadError> {
        let end = range.end_offset().ok_or(ReadError::OffsetOverflow {
            offset: range.offset,
            length: range.length,
        })?;

        if end > source.size_bytes() {
            return Err(ReadError::InvalidRange {
                offset: range.offset,
                length: range.length,
                size: source.size_bytes(),
            });
        }

        if chunk_size == 0 {
            return Err(ReadError::EmptyBuffer);
        }

        Ok(Self {
            source,
            range,
            chunk_size,
            cursor: range.offset,
        })
    }

    pub fn next_chunk(&mut self) -> Result<Option<Chunk>, ReadError> {
        let end = self.range.offset + self.range.length;
        if self.cursor >= end {
            return Ok(None);
        }

        let remaining = (end - self.cursor) as usize;
        let read_len = self.chunk_size.min(remaining);
        let mut data = vec![0_u8; read_len];
        let result = self.source.read_at(self.cursor, &mut data)?;
        data.truncate(result.bytes_read);

        let chunk = Chunk {
            offset: self.cursor,
            data,
        };
        self.cursor += result.bytes_read as u64;

        Ok(Some(chunk))
    }
}

pub fn hash_range<S: BlockSource + ?Sized>(
    source: &S,
    range: ByteRange,
    chunk_size: usize,
) -> Result<ReadSummary, ReadError> {
    let mut reader = ChunkedReader::new(source, range, chunk_size)?;
    let mut hasher = Sha256::new();
    let mut bytes_read = 0_u64;
    let mut chunks_read = 0_u64;

    while let Some(chunk) = reader.next_chunk()? {
        if chunk.data.is_empty() {
            break;
        }
        hasher.update(&chunk.data);
        bytes_read += chunk.data.len() as u64;
        chunks_read += 1;
    }

    Ok(ReadSummary {
        range,
        chunk_size,
        bytes_read,
        chunks_read,
        sha256: hasher.finalize_hex(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::FileBlockSource;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_file_name() -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("dr_core_pipeline_{}_{id}.bin", std::process::id()))
    }

    #[test]
    fn hashes_range_in_chunks() {
        let path = temp_file_name();
        {
            let mut file = File::create(&path).unwrap();
            file.write_all(b"0123456789abcdef").unwrap();
        }

        let source = FileBlockSource::open(&path).unwrap();
        let summary = hash_range(&source, ByteRange::new(4, 8), 3).unwrap();

        assert_eq!(summary.bytes_read, 8);
        assert_eq!(summary.chunks_read, 3);
        assert_eq!(summary.sha256, crate::sha256_hex(b"456789ab"));

        std::fs::remove_file(path).unwrap();
    }
}
