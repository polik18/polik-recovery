use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhysicalDevice {
    pub id: String,
    pub display_name: String,
    pub platform_path: String,
    pub size_bytes: u64,
    pub logical_sector_size: u32,
    pub physical_sector_size: u32,
    pub is_removable: bool,
    pub is_system_disk: bool,
    pub bus_type: BusType,
    pub trim_supported: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusType {
    Unknown,
    Ata,
    Sata,
    Nvme,
    Usb,
    Thunderbolt,
    Virtual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub offset: u64,
    pub length: u64,
}

impl ByteRange {
    pub fn new(offset: u64, length: u64) -> Self {
        Self { offset, length }
    }

    pub fn end_offset(self) -> Option<u64> {
        self.offset.checked_add(self.length)
    }
}

#[derive(Debug)]
pub enum ReadError {
    Io(std::io::Error),
    InvalidRange { offset: u64, length: u64, size: u64 },
    OffsetOverflow { offset: u64, length: u64 },
    EmptyBuffer,
    LockPoisoned,
    UnsupportedSourceType { path: PathBuf },
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::InvalidRange {
                offset,
                length,
                size,
            } => write!(
                f,
                "read range is outside source: offset={offset}, length={length}, size={size}"
            ),
            Self::OffsetOverflow { offset, length } => {
                write!(
                    f,
                    "read range overflows u64: offset={offset}, length={length}"
                )
            }
            Self::EmptyBuffer => write!(f, "read buffer must not be empty"),
            Self::LockPoisoned => write!(f, "internal source lock was poisoned"),
            Self::UnsupportedSourceType { path } => write!(
                f,
                "RAW_DEVICE_UNSUPPORTED: file-backed sources must be regular files: {}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ReadError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadResult {
    pub offset: u64,
    pub bytes_read: usize,
}

pub trait BlockSource: Send + Sync {
    fn device(&self) -> &PhysicalDevice;
    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<ReadResult, ReadError>;

    fn size_bytes(&self) -> u64 {
        self.device().size_bytes
    }

    fn sector_size(&self) -> u32 {
        self.device().logical_sector_size
    }
}

#[derive(Debug)]
pub struct FileBlockSource {
    device: PhysicalDevice,
    file: Mutex<File>,
    path: PathBuf,
}

fn open_regular_file_source(path: &Path) -> Result<File, ReadError> {
    let text = path.to_string_lossy();
    if text.starts_with("/dev/") || text.starts_with(r"\\.\") || text.starts_with(r"\\?\") {
        return Err(ReadError::UnsupportedSourceType {
            path: path.to_path_buf(),
        });
    }

    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT, GetFileInformationByHandle,
        };

        let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
        let ok =
            unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, info.as_mut_ptr()) };
        if ok == 0 {
            return Err(ReadError::Io(std::io::Error::last_os_error()));
        }
        let info = unsafe { info.assume_init() };
        if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(ReadError::UnsupportedSourceType {
                path: path.to_path_buf(),
            });
        }
    }
    if !file.metadata()?.file_type().is_file() {
        return Err(ReadError::UnsupportedSourceType {
            path: path.to_path_buf(),
        });
    }
    Ok(file)
}

impl FileBlockSource {
    /// Open a regular image file with an explicit trusted size.
    pub fn open_with_known_size(
        path: impl AsRef<Path>,
        size_bytes: u64,
    ) -> Result<Self, ReadError> {
        let path = path.as_ref().to_path_buf();
        let file = open_regular_file_source(&path)?;
        let display_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("device")
            .to_string();
        Ok(Self {
            device: PhysicalDevice {
                id: format!("file:{}", path.display()),
                display_name,
                platform_path: path.display().to_string(),
                size_bytes,
                logical_sector_size: 512,
                physical_sector_size: 512,
                is_removable: false,
                is_system_disk: false,
                bus_type: BusType::Virtual,
                trim_supported: None,
            },
            file: Mutex::new(file),
            path,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, ReadError> {
        let path = path.as_ref().to_path_buf();
        let file = open_regular_file_source(&path)?;
        let size_bytes = file.metadata()?.len();
        let display_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("file-backed-source")
            .to_string();

        Ok(Self {
            device: PhysicalDevice {
                id: format!("file:{}", path.display()),
                display_name,
                platform_path: path.display().to_string(),
                size_bytes,
                logical_sector_size: 512,
                physical_sector_size: 512,
                is_removable: false,
                is_system_disk: false,
                bus_type: BusType::Virtual,
                trim_supported: None,
            },
            file: Mutex::new(file),
            path,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    #[cfg(unix)]
    pub fn with_raw_fd<T>(
        &self,
        operation: impl FnOnce(std::os::fd::RawFd) -> T,
    ) -> Result<T, ReadError> {
        use std::os::fd::AsRawFd;

        let file = self.file.lock().map_err(|_| ReadError::LockPoisoned)?;
        Ok(operation(file.as_raw_fd()))
    }

    #[cfg(windows)]
    pub fn with_raw_handle<T>(
        &self,
        operation: impl FnOnce(std::os::windows::io::RawHandle) -> T,
    ) -> Result<T, ReadError> {
        use std::os::windows::io::AsRawHandle;

        let file = self.file.lock().map_err(|_| ReadError::LockPoisoned)?;
        Ok(operation(file.as_raw_handle()))
    }

    fn validate_range(&self, offset: u64, length: usize) -> Result<(), ReadError> {
        if length == 0 {
            return Err(ReadError::EmptyBuffer);
        }

        let length = length as u64;
        let end = offset
            .checked_add(length)
            .ok_or(ReadError::OffsetOverflow { offset, length })?;

        if offset > self.device.size_bytes || end > self.device.size_bytes {
            return Err(ReadError::InvalidRange {
                offset,
                length,
                size: self.device.size_bytes,
            });
        }

        Ok(())
    }
}

impl BlockSource for FileBlockSource {
    fn device(&self) -> &PhysicalDevice {
        &self.device
    }

    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<ReadResult, ReadError> {
        self.validate_range(offset, buffer.len())?;

        let mut file = self.file.lock().map_err(|_| ReadError::LockPoisoned)?;
        file.seek(SeekFrom::Start(offset))?;
        let bytes_read = file.read(buffer)?;

        Ok(ReadResult { offset, bytes_read })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_file_name() -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("dr_core_block_{}_{id}.bin", std::process::id()))
    }

    #[test]
    fn reads_from_file_backed_source() {
        let path = temp_file_name();
        {
            let mut file = File::create(&path).unwrap();
            file.write_all(b"0123456789abcdef").unwrap();
        }

        let source = FileBlockSource::open(&path).unwrap();
        let mut buffer = [0_u8; 4];
        let result = source.read_at(4, &mut buffer).unwrap();

        assert_eq!(result.bytes_read, 4);
        assert_eq!(&buffer, b"4567");

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_out_of_range_reads() {
        let path = temp_file_name();
        {
            let mut file = File::create(&path).unwrap();
            file.write_all(b"abcd").unwrap();
        }

        let source = FileBlockSource::open(&path).unwrap();
        let mut buffer = [0_u8; 8];

        assert!(matches!(
            source.read_at(0, &mut buffer),
            Err(ReadError::InvalidRange { .. })
        ));

        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_raw_device_paths_before_open() {
        let mac_error = FileBlockSource::open("/dev/disk999").unwrap_err();
        let windows_error = FileBlockSource::open(r"\\.\PhysicalDrive999").unwrap_err();
        assert!(matches!(mac_error, ReadError::UnsupportedSourceType { .. }));
        assert!(matches!(
            windows_error,
            ReadError::UnsupportedSourceType { .. }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_link_source() {
        use std::os::unix::fs::symlink;

        let path = temp_file_name();
        let link = path.with_extension("link");
        std::fs::write(&path, b"fixture").unwrap();
        symlink(&path, &link).unwrap();

        assert!(FileBlockSource::open(&link).is_err());

        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(path).unwrap();
    }
}
