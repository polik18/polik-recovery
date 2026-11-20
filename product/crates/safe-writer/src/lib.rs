use data_recovery_core::{FileBlockSource, Sha256, sha256_hex};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

const MIN_FREE_SPACE_MARGIN_BYTES: u64 = 64 * 1024 * 1024;
const MANIFEST_SPACE_RESERVE_BYTES: u64 = 64 * 1024;

#[cfg(test)]
thread_local! {
    static TEST_DISK_FULL_AFTER: std::cell::Cell<Option<usize>> = const {
        std::cell::Cell::new(None)
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageIdentity {
    pub label: String,
    pub physical_stores: BTreeSet<String>,
    provenance: StorageIdentityProvenance,
    #[cfg(any(target_os = "macos", windows))]
    file_object: Option<FileObjectIdentity>,
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileObjectIdentity {
    device: u64,
    inode: u64,
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileObjectIdentity {
    volume_serial: u64,
    file_index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StorageIdentityProvenance {
    Unverified,
    #[cfg(target_os = "macos")]
    MacOsResolved,
    #[cfg(windows)]
    WindowsResolved,
    #[cfg(debug_assertions)]
    DebugFixture,
}

impl StorageIdentity {
    pub fn from_physical_stores<I, S>(label: impl Into<String>, stores: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let physical_stores = stores
            .into_iter()
            .map(Into::into)
            .filter(|store: &String| !store.trim().is_empty())
            .collect();
        Self {
            label: label.into(),
            physical_stores,
            provenance: StorageIdentityProvenance::Unverified,
            #[cfg(any(target_os = "macos", windows))]
            file_object: None,
        }
    }

    pub fn unknown(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            physical_stores: BTreeSet::new(),
            provenance: StorageIdentityProvenance::Unverified,
            #[cfg(any(target_os = "macos", windows))]
            file_object: None,
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_resolved<I, S>(
        label: impl Into<String>,
        stores: I,
        file_object: Option<FileObjectIdentity>,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut identity = Self::from_physical_stores(label, stores);
        identity.provenance = StorageIdentityProvenance::MacOsResolved;
        identity.file_object = file_object;
        identity
    }

    #[cfg(windows)]
    fn windows_resolved<I, S>(
        label: impl Into<String>,
        stores: I,
        file_object: Option<FileObjectIdentity>,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut identity = Self::from_physical_stores(label, stores);
        identity.provenance = StorageIdentityProvenance::WindowsResolved;
        identity.file_object = file_object;
        identity
    }

    #[cfg(debug_assertions)]
    pub fn debug_fixture<I, S>(label: impl Into<String>, stores: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut identity = Self::from_physical_stores(label, stores);
        identity.provenance = StorageIdentityProvenance::DebugFixture;
        identity
    }

    pub fn is_complete(&self) -> bool {
        !self.physical_stores.is_empty()
    }
}

#[derive(Debug)]
pub enum SafeWriterError {
    Io(std::io::Error),
    UnknownStorageIdentity {
        label: String,
    },
    SharedPhysicalStorage {
        stores: Vec<String>,
    },
    OutputEqualsSource,
    OutputInsideSourceDirectory {
        source_parent: PathBuf,
        output: PathBuf,
    },
    MissingOutputParent(PathBuf),
    OutputPathContainsSymlink(PathBuf),
    OutputDirectoryChanged(PathBuf),
    UnboundStorageIdentity {
        label: String,
    },
    OutputStorageIdentityChanged {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    SourceStorageIdentityChanged {
        expected: Vec<String>,
        actual: Vec<String>,
    },
    SourceFileIdentityChanged {
        expected_device: u64,
        expected_inode: u64,
        actual_device: u64,
        actual_inode: u64,
    },
    InsufficientFreeSpace {
        available_bytes: u64,
        required_bytes: u64,
    },
    StorageIdentityResolution(String),
    OutputPairNeedsManualRecovery(String),
    UnsupportedStorageIdentityPlatform,
}

impl std::fmt::Display for SafeWriterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::UnknownStorageIdentity { label } => {
                write!(f, "physical storage identity is unknown for {label}")
            }
            Self::SharedPhysicalStorage { stores } => write!(
                f,
                "source and output share physical storage: {}",
                stores.join(", ")
            ),
            Self::OutputEqualsSource => write!(f, "output path must not equal source path"),
            Self::OutputInsideSourceDirectory {
                source_parent,
                output,
            } => write!(
                f,
                "output path {} must not be inside source directory {}",
                output.display(),
                source_parent.display()
            ),
            Self::MissingOutputParent(path) => {
                write!(
                    f,
                    "output parent directory does not exist: {}",
                    path.display()
                )
            }
            Self::OutputPathContainsSymlink(path) => write!(
                f,
                "output directory path must not contain symbolic links: {}",
                path.display()
            ),
            Self::OutputDirectoryChanged(path) => write!(
                f,
                "output directory changed while it was being opened: {}",
                path.display()
            ),
            Self::UnboundStorageIdentity { label } => write!(
                f,
                "storage identity was not produced by a platform resolver: {label}"
            ),
            Self::OutputStorageIdentityChanged { expected, actual } => write!(
                f,
                "opened output directory storage changed (expected: {}; actual: {})",
                expected.join(", "),
                actual.join(", ")
            ),
            Self::SourceStorageIdentityChanged { expected, actual } => write!(
                f,
                "opened source file storage changed (expected: {}; actual: {})",
                expected.join(", "),
                actual.join(", ")
            ),
            Self::SourceFileIdentityChanged {
                expected_device,
                expected_inode,
                actual_device,
                actual_inode,
            } => write!(
                f,
                "opened source file identity changed (expected device/inode: {expected_device}/{expected_inode}; actual: {actual_device}/{actual_inode})"
            ),
            Self::InsufficientFreeSpace {
                available_bytes,
                required_bytes,
            } => write!(
                f,
                "insufficient output space: {available_bytes} bytes available, {required_bytes} bytes required",
            ),
            Self::StorageIdentityResolution(message) => {
                write!(f, "storage identity resolution failed: {message}")
            }
            Self::OutputPairNeedsManualRecovery(message) => {
                write!(
                    f,
                    "recovery output pair needs manual reconciliation: {message}"
                )
            }
            Self::UnsupportedStorageIdentityPlatform => write!(
                f,
                "physical-store identity resolution is not implemented on this platform"
            ),
        }
    }
}

impl std::error::Error for SafeWriterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SafeWriterError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryManifest {
    pub source: PathBuf,
    pub output: PathBuf,
    pub manifest_path: PathBuf,
    pub bytes_written: usize,
    pub sha256: String,
    pub source_record: Option<u64>,
}

#[derive(Debug)]
pub struct VerifiedSourceStorage<'a> {
    identity: &'a StorageIdentity,
    _source: &'a FileBlockSource,
}

pub fn verify_open_source_storage<'a>(
    source: &'a FileBlockSource,
    expected: &'a StorageIdentity,
) -> Result<VerifiedSourceStorage<'a>, SafeWriterError> {
    #[cfg(debug_assertions)]
    if expected.provenance == StorageIdentityProvenance::DebugFixture {
        return Ok(VerifiedSourceStorage {
            identity: expected,
            _source: source,
        });
    }

    #[cfg(target_os = "macos")]
    {
        if expected.provenance != StorageIdentityProvenance::MacOsResolved {
            return Err(SafeWriterError::UnboundStorageIdentity {
                label: expected.label.clone(),
            });
        }
        source
            .with_raw_fd(|fd| verify_opened_source_fd(fd, expected))
            .map_err(|error| {
                SafeWriterError::StorageIdentityResolution(format!(
                    "cannot inspect opened source file: {error}"
                ))
            })??;
        Ok(VerifiedSourceStorage {
            identity: expected,
            _source: source,
        })
    }

    #[cfg(windows)]
    {
        if expected.provenance != StorageIdentityProvenance::WindowsResolved {
            return Err(SafeWriterError::UnboundStorageIdentity {
                label: expected.label.clone(),
            });
        }
        source
            .with_raw_handle(|handle| verify_opened_source_handle(handle as _, expected))
            .map_err(|error| {
                SafeWriterError::StorageIdentityResolution(format!(
                    "cannot inspect opened source file: {error}"
                ))
            })??;
        Ok(VerifiedSourceStorage {
            identity: expected,
            _source: source,
        })
    }

    #[cfg(not(any(target_os = "macos", windows)))]
    Err(SafeWriterError::UnsupportedStorageIdentityPlatform)
}

pub fn validate_storage_isolation(
    source: &StorageIdentity,
    output: &StorageIdentity,
) -> Result<(), SafeWriterError> {
    if !source.is_complete() {
        return Err(SafeWriterError::UnknownStorageIdentity {
            label: source.label.clone(),
        });
    }
    if !output.is_complete() {
        return Err(SafeWriterError::UnknownStorageIdentity {
            label: output.label.clone(),
        });
    }

    let shared = source
        .physical_stores
        .intersection(&output.physical_stores)
        .cloned()
        .collect::<Vec<_>>();
    if !shared.is_empty() {
        return Err(SafeWriterError::SharedPhysicalStorage { stores: shared });
    }

    Ok(())
}

/// Resolve the physical stores hosting a regular source image and an output path.
///
/// The output itself may not exist yet; its nearest existing ancestor is resolved.
/// Unsupported platforms and incomplete identities fail closed.
#[cfg(target_os = "macos")]
pub fn resolve_image_storage_identities(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(StorageIdentity, StorageIdentity), SafeWriterError> {
    use std::os::unix::fs::MetadataExt;

    let source = source.as_ref();
    let output = output.as_ref();
    reject_raw_device_path(source)?;
    reject_output_path_components(output)?;
    let source_path = std::fs::canonicalize(source).map_err(|error| {
        SafeWriterError::StorageIdentityResolution(format!(
            "cannot canonicalize source {}: {error}",
            source.display()
        ))
    })?;
    let source_metadata = std::fs::metadata(&source_path)?;
    if !source_metadata.file_type().is_file() {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "source is not a regular image file: {}",
            source_path.display()
        )));
    }
    let output_parent = output.parent().ok_or_else(|| {
        SafeWriterError::StorageIdentityResolution(format!(
            "output has no parent directory: {}",
            output.display()
        ))
    })?;
    let output_path = nearest_existing_ancestor(output_parent)?;
    let source_stores = macos_physical_stores_for_path(&source_path)?;
    let output_stores = macos_physical_stores_for_path(&output_path)?;
    let source_identity = StorageIdentity::macos_resolved(
        source_path.display().to_string(),
        source_stores,
        Some(FileObjectIdentity {
            device: source_metadata.dev(),
            inode: source_metadata.ino(),
        }),
    );
    let output_identity =
        StorageIdentity::macos_resolved(output_path.display().to_string(), output_stores, None);
    validate_storage_isolation(&source_identity, &output_identity)?;
    Ok((source_identity, output_identity))
}

#[cfg(windows)]
pub fn resolve_image_storage_identities(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
) -> Result<(StorageIdentity, StorageIdentity), SafeWriterError> {
    use std::os::windows::io::AsRawHandle;

    let source = source.as_ref();
    let output = output.as_ref();
    reject_raw_device_path(source)?;
    reject_output_path_components(output)?;
    let source_path = std::fs::canonicalize(source).map_err(|error| {
        SafeWriterError::StorageIdentityResolution(format!(
            "cannot canonicalize source {}: {error}",
            source.display()
        ))
    })?;
    let source_file = open_windows_path_handle(&source_path, false)?;
    ensure_windows_regular_file(&source_file, &source_path)?;

    let output_parent = output.parent().ok_or_else(|| {
        SafeWriterError::StorageIdentityResolution(format!(
            "output has no parent directory: {}",
            output.display()
        ))
    })?;
    let output_path = nearest_existing_ancestor(output_parent)?;
    let output_directory = open_windows_path_handle(&output_path, true)?;
    ensure_windows_directory(&output_directory, &output_path)?;

    let source_handle = source_file.as_raw_handle() as _;
    let output_handle = output_directory.as_raw_handle() as _;
    let source_identity = StorageIdentity::windows_resolved(
        source_path.display().to_string(),
        windows_physical_stores_for_handle(source_handle)?,
        Some(windows_file_object_identity(source_handle)?),
    );
    let output_identity = StorageIdentity::windows_resolved(
        output_path.display().to_string(),
        windows_physical_stores_for_handle(output_handle)?,
        if output_path == std::fs::canonicalize(output_parent).unwrap_or_default() {
            Some(windows_file_object_identity(output_handle)?)
        } else {
            None
        },
    );
    validate_storage_isolation(&source_identity, &output_identity)?;
    Ok((source_identity, output_identity))
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn resolve_image_storage_identities(
    _source: impl AsRef<Path>,
    _output: impl AsRef<Path>,
) -> Result<(StorageIdentity, StorageIdentity), SafeWriterError> {
    Err(SafeWriterError::UnsupportedStorageIdentityPlatform)
}

fn reject_raw_device_path(path: &Path) -> Result<(), SafeWriterError> {
    let text = path.to_string_lossy();
    if text.starts_with("/dev/") || text.starts_with(r"\\.\") || text.starts_with(r"\\?\") {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "RAW_DEVICE_UNSUPPORTED: expected a regular image file, got {}",
            path.display()
        )));
    }
    Ok(())
}

fn nearest_existing_ancestor(path: &Path) -> Result<PathBuf, SafeWriterError> {
    let mut candidate = path;
    loop {
        if candidate.exists() {
            return std::fs::canonicalize(candidate).map_err(|error| {
                SafeWriterError::StorageIdentityResolution(format!(
                    "cannot canonicalize output ancestor {}: {error}",
                    candidate.display()
                ))
            });
        }
        candidate = candidate.parent().ok_or_else(|| {
            SafeWriterError::StorageIdentityResolution(format!(
                "cannot find an existing ancestor for output path {}",
                path.display()
            ))
        })?;
    }
}

#[cfg(target_os = "macos")]
fn macos_whole_disk_identifier(value: &str) -> Option<String> {
    let value = value.strip_prefix("/dev/").unwrap_or(value);
    let rest = value.strip_prefix("disk")?;
    let digit_count = rest.bytes().take_while(u8::is_ascii_digit).count();
    if digit_count == 0 {
        return None;
    }
    let mut remaining = &rest[digit_count..];
    while !remaining.is_empty() {
        remaining = remaining.strip_prefix('s')?;
        let count = remaining.bytes().take_while(u8::is_ascii_digit).count();
        if count == 0 {
            return None;
        }
        remaining = &remaining[count..];
    }
    Some(format!("disk{}", &rest[..digit_count]))
}

#[cfg(target_os = "macos")]
fn macos_collect_disk_identifiers(value: &plist::Value, stores: &mut BTreeSet<String>) {
    match value {
        plist::Value::String(value) => {
            if let Some(identifier) = macos_whole_disk_identifier(value) {
                stores.insert(identifier);
            }
        }
        plist::Value::Array(values) => {
            for value in values {
                macos_collect_disk_identifiers(value, stores);
            }
        }
        plist::Value::Dictionary(values) => {
            for value in values.values() {
                macos_collect_disk_identifiers(value, stores);
            }
        }
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn macos_physical_stores_from_plist(value: &plist::Value) -> BTreeSet<String> {
    let Some(info) = value.as_dictionary() else {
        return BTreeSet::new();
    };
    let mut stores = BTreeSet::new();
    if let Some(value) = info.get("APFSPhysicalStores") {
        macos_collect_disk_identifiers(value, &mut stores);
    }
    if !stores.is_empty() {
        return stores;
    }
    if let Some(parent) = info
        .get("ParentWholeDisk")
        .and_then(plist::Value::as_string)
    {
        if let Some(identifier) = macos_whole_disk_identifier(parent) {
            stores.insert(identifier);
            return stores;
        }
    }
    if info
        .get("WholeDisk")
        .and_then(plist::Value::as_boolean)
        .unwrap_or(false)
    {
        if let Some(device) = info
            .get("DeviceIdentifier")
            .and_then(plist::Value::as_string)
        {
            if let Some(identifier) = macos_whole_disk_identifier(device) {
                stores.insert(identifier);
            }
        }
    }
    stores
}

#[cfg(target_os = "macos")]
fn macos_device_from_df_output(output: &str) -> Result<String, SafeWriterError> {
    let line = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .next_back()
        .ok_or_else(|| {
            SafeWriterError::StorageIdentityResolution(
                "df returned no parseable mount information".to_string(),
            )
        })?;
    let device = line.split_whitespace().next().ok_or_else(|| {
        SafeWriterError::StorageIdentityResolution("df returned no filesystem source".to_string())
    })?;
    if !device.starts_with("/dev/") {
        return Err(SafeWriterError::StorageIdentityResolution(
            "df did not return a local block device".to_string(),
        ));
    }
    Ok(device.to_string())
}

#[cfg(target_os = "macos")]
fn macos_diskutil_lookup_target(path: &Path) -> Result<String, SafeWriterError> {
    let output = std::process::Command::new("/bin/df")
        .arg("-P")
        .arg(path)
        .output()
        .map_err(SafeWriterError::Io)?;
    if !output.status.success() {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "df cannot resolve {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    macos_device_from_df_output(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "macos")]
fn macos_diskutil_info_plist(target: &str) -> Result<plist::Value, SafeWriterError> {
    let output = std::process::Command::new("/usr/sbin/diskutil")
        .args(["info", "-plist", target])
        .output()
        .map_err(SafeWriterError::Io)?;
    if !output.status.success() {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "diskutil cannot resolve {target}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    plist::Value::from_reader(std::io::Cursor::new(output.stdout)).map_err(|error| {
        SafeWriterError::StorageIdentityResolution(format!(
            "diskutil returned invalid plist for {target}: {error}"
        ))
    })
}

#[cfg(target_os = "macos")]
fn macos_physical_stores_for_path(path: &Path) -> Result<BTreeSet<String>, SafeWriterError> {
    let target = macos_diskutil_lookup_target(path)?;
    macos_physical_stores_for_target(&target)
}

#[cfg(target_os = "macos")]
fn macos_physical_stores_for_target(target: &str) -> Result<BTreeSet<String>, SafeWriterError> {
    let info = macos_diskutil_info_plist(target)?;
    let mut stores = macos_physical_stores_from_plist(&info);
    if let Some(container) = info
        .as_dictionary()
        .and_then(|dict| dict.get("APFSContainerReference"))
        .and_then(plist::Value::as_string)
    {
        stores.extend(macos_physical_stores_from_plist(
            &macos_diskutil_info_plist(container)?,
        ));
    }
    if stores.is_empty() {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "cannot prove physical storage for {target}"
        )));
    }
    Ok(stores)
}

#[cfg(windows)]
fn open_windows_path_handle(path: &Path, directory: bool) -> Result<File, SafeWriterError> {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    };

    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    let mut flags = FILE_FLAG_OPEN_REPARSE_POINT;
    if directory {
        flags |= FILE_FLAG_BACKUP_SEMANTICS;
    }
    options.custom_flags(flags);
    Ok(options.open(path)?)
}

#[cfg(windows)]
fn windows_handle_information(
    handle: windows_sys::Win32::Foundation::HANDLE,
) -> Result<windows_sys::Win32::Storage::FileSystem::BY_HANDLE_FILE_INFORMATION, SafeWriterError> {
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };

    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
    if unsafe { GetFileInformationByHandle(handle, information.as_mut_ptr()) } == 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    Ok(unsafe { information.assume_init() })
}

#[cfg(windows)]
fn ensure_windows_regular_file(file: &File, path: &Path) -> Result<(), SafeWriterError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    };

    let information = windows_handle_information(file.as_raw_handle() as _)?;
    if information.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0
    {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "source is not a non-reparse regular image file: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn ensure_windows_directory(file: &File, path: &Path) -> Result<(), SafeWriterError> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    };

    let information = windows_handle_information(file.as_raw_handle() as _)?;
    if information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(SafeWriterError::OutputPathContainsSymlink(
            path.to_path_buf(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn windows_file_object_identity(
    handle: windows_sys::Win32::Foundation::HANDLE,
) -> Result<FileObjectIdentity, SafeWriterError> {
    let information = windows_handle_information(handle)?;
    Ok(FileObjectIdentity {
        volume_serial: u64::from(information.dwVolumeSerialNumber),
        file_index: (u64::from(information.nFileIndexHigh) << 32)
            | u64::from(information.nFileIndexLow),
    })
}

#[cfg(windows)]
fn windows_final_path_for_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
) -> Result<PathBuf, SafeWriterError> {
    use std::os::windows::ffi::OsStringExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_NAME_NORMALIZED, GetFinalPathNameByHandleW, VOLUME_NAME_DOS,
    };

    let flags = FILE_NAME_NORMALIZED | VOLUME_NAME_DOS;
    let required = unsafe { GetFinalPathNameByHandleW(handle, std::ptr::null_mut(), 0, flags) };
    if required == 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    let mut buffer = vec![0u16; required as usize + 1];
    let written = unsafe {
        GetFinalPathNameByHandleW(handle, buffer.as_mut_ptr(), buffer.len() as u32, flags)
    };
    if written == 0 || written as usize >= buffer.len() {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    buffer.truncate(written as usize);
    Ok(PathBuf::from(std::ffi::OsString::from_wide(&buffer)))
}

#[cfg(windows)]
fn windows_volume_name_for_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
) -> Result<Vec<u16>, SafeWriterError> {
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows_sys::Win32::Storage::FileSystem::{
        GetVolumeNameForVolumeMountPointW, GetVolumePathNameW,
    };

    let final_path = windows_final_path_for_handle(handle)?;
    let final_wide = final_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut root = vec![0u16; 1024];
    let mut ok =
        unsafe { GetVolumePathNameW(final_wide.as_ptr(), root.as_mut_ptr(), root.len() as u32) };
    if ok == 0 {
        let text = final_path.to_string_lossy();
        let ordinary = text.strip_prefix(r"\\?\").unwrap_or(&text);
        let ordinary_wide = std::ffi::OsStr::new(ordinary)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        ok = unsafe {
            GetVolumePathNameW(ordinary_wide.as_ptr(), root.as_mut_ptr(), root.len() as u32)
        };
    }
    if ok == 0 {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "cannot resolve a local volume for {}: {}",
            final_path.display(),
            std::io::Error::last_os_error()
        )));
    }
    let root_len = root
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(root.len());
    root.truncate(root_len + 1);

    let mut volume = vec![0u16; 1024];
    if unsafe {
        GetVolumeNameForVolumeMountPointW(root.as_ptr(), volume.as_mut_ptr(), volume.len() as u32)
    } == 0
    {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "cannot resolve volume GUID for {}: {}",
            std::ffi::OsString::from_wide(&root[..root_len]).to_string_lossy(),
            std::io::Error::last_os_error()
        )));
    }
    let volume_len = volume
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(volume.len());
    volume.truncate(volume_len);
    while volume.last().copied() == Some('\\' as u16) {
        volume.pop();
    }
    volume.push(0);
    Ok(volume)
}

#[cfg(windows)]
fn windows_physical_stores_for_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
) -> Result<BTreeSet<String>, SafeWriterError> {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::Foundation::{ERROR_MORE_DATA, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
        IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    use windows_sys::Win32::System::Ioctl::VOLUME_DISK_EXTENTS;

    let volume_name = windows_volume_name_for_handle(handle)?;
    let volume_handle = unsafe {
        CreateFileW(
            volume_name.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if volume_handle == INVALID_HANDLE_VALUE {
        return Err(SafeWriterError::StorageIdentityResolution(format!(
            "cannot open resolved volume: {}",
            std::io::Error::last_os_error()
        )));
    }
    let volume_file = unsafe { File::from_raw_handle(volume_handle as _) };

    let mut buffer_size = 4096usize;
    loop {
        let mut buffer = vec![0u8; buffer_size];
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                volume_handle,
                IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS,
                std::ptr::null(),
                0,
                buffer.as_mut_ptr().cast(),
                buffer.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok != 0 {
            let extents = unsafe { &*(buffer.as_ptr().cast::<VOLUME_DISK_EXTENTS>()) };
            let count = extents.NumberOfDiskExtents as usize;
            let minimum = std::mem::size_of::<VOLUME_DISK_EXTENTS>().saturating_add(
                count.saturating_sub(1) * std::mem::size_of_val(&extents.Extents[0]),
            );
            if count == 0 || minimum > buffer.len() || minimum > returned as usize {
                return Err(SafeWriterError::StorageIdentityResolution(
                    "volume returned an invalid or empty physical extent list".to_string(),
                ));
            }
            let values = unsafe { std::slice::from_raw_parts(extents.Extents.as_ptr(), count) };
            let stores = values
                .iter()
                .map(|extent| format!("disk:{}", extent.DiskNumber))
                .collect::<BTreeSet<_>>();
            drop(volume_file);
            return Ok(stores);
        }

        let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        if error != ERROR_MORE_DATA || buffer_size >= 1024 * 1024 {
            return Err(SafeWriterError::StorageIdentityResolution(format!(
                "cannot query physical disk extents for the resolved volume: {}",
                std::io::Error::from_raw_os_error(error as i32)
            )));
        }
        buffer_size *= 2;
    }
}

pub fn write_recovered_file_with_storage_identities(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
    data: &[u8],
    source_record: Option<u64>,
    source_storage: &VerifiedSourceStorage<'_>,
    output_identity: &StorageIdentity,
) -> Result<RecoveryManifest, SafeWriterError> {
    let source_identity = source_storage.identity;
    validate_storage_isolation(source_identity, output_identity)?;
    validate_write_identity_provenance(source_identity, output_identity)?;
    write_recovered_file_inner(source, output, data, source_record, Some(output_identity))
}

#[cfg(test)]
fn write_recovered_file(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
    data: &[u8],
    source_record: Option<u64>,
) -> Result<RecoveryManifest, SafeWriterError> {
    write_recovered_file_inner(source, output, data, source_record, None)
}

fn write_recovered_file_inner(
    source: impl AsRef<Path>,
    output: impl AsRef<Path>,
    data: &[u8],
    source_record: Option<u64>,
    output_identity: Option<&StorageIdentity>,
) -> Result<RecoveryManifest, SafeWriterError> {
    let source = source.as_ref();
    let output = output.as_ref();
    reject_output_path_components(output)?;
    reconcile_recovery_output(output)?;
    validate_output_path(source, output)?;
    let output_name = output.file_name().ok_or_else(|| {
        SafeWriterError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("output path has no file name: {}", output.display()),
        ))
    })?;
    let output = canonical_parent(output)?.join(output_name);
    validate_output_capacity(&output, data.len())?;

    let sha256 = sha256_hex(data);
    let manifest_path = manifest_path_for(&output);
    let output_partial = partial_path_for(&output);
    let manifest_partial = partial_path_for(&manifest_path);

    let manifest = RecoveryManifest {
        source: source.to_path_buf(),
        output: output.clone(),
        manifest_path: manifest_path.clone(),
        bytes_written: data.len(),
        sha256,
        source_record,
    };
    write_output_pair(
        &manifest,
        data,
        &output_partial,
        &manifest_partial,
        output_identity,
    )?;

    Ok(manifest)
}

fn write_recovered_data(file: &mut File, data: &[u8]) -> Result<(), SafeWriterError> {
    #[cfg(test)]
    {
        let injected = TEST_DISK_FULL_AFTER.with(|limit| limit.get());
        if let Some(limit) = injected {
            let prefix = limit.min(data.len());
            file.write_all(&data[..prefix])?;
            return Err(SafeWriterError::Io(std::io::Error::other(
                "injected disk-full failure",
            )));
        }
    }
    file.write_all(data)?;
    Ok(())
}

fn validate_write_identity_provenance(
    source: &StorageIdentity,
    output: &StorageIdentity,
) -> Result<(), SafeWriterError> {
    #[cfg(target_os = "macos")]
    if matches!(
        (&source.provenance, &output.provenance),
        (
            StorageIdentityProvenance::MacOsResolved,
            StorageIdentityProvenance::MacOsResolved
        )
    ) {
        return Ok(());
    }

    #[cfg(windows)]
    if matches!(
        (&source.provenance, &output.provenance),
        (
            StorageIdentityProvenance::WindowsResolved,
            StorageIdentityProvenance::WindowsResolved
        )
    ) {
        return Ok(());
    }

    #[cfg(debug_assertions)]
    if matches!(
        (&source.provenance, &output.provenance),
        (
            StorageIdentityProvenance::DebugFixture,
            StorageIdentityProvenance::DebugFixture
        )
    ) {
        return Ok(());
    }

    let label = if source.provenance == StorageIdentityProvenance::Unverified {
        source.label.clone()
    } else {
        output.label.clone()
    };
    Err(SafeWriterError::UnboundStorageIdentity { label })
}

#[cfg(unix)]
fn write_output_pair(
    manifest: &RecoveryManifest,
    data: &[u8],
    output_partial: &Path,
    manifest_partial: &Path,
    output_identity: Option<&StorageIdentity>,
) -> Result<(), SafeWriterError> {
    let output_parent = manifest
        .output
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(manifest.output.clone()))?;
    let directory = open_verified_output_directory(output_parent)?;
    #[cfg(target_os = "macos")]
    if let Some(identity) = output_identity {
        verify_opened_output_storage(&directory, identity)?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = output_identity;

    let mut output_file = create_new_at(&directory, file_name(output_partial)?)?;
    write_recovered_data(&mut output_file, data)?;
    output_file.sync_all()?;

    let mut manifest_file = create_new_at(&directory, file_name(manifest_partial)?)?;
    write_manifest_contents(manifest, &mut manifest_file)?;
    manifest_file.sync_all()?;

    commit_no_replace_at(
        &directory,
        file_name(output_partial)?,
        file_name(&manifest.output)?,
    )?;
    commit_no_replace_at(
        &directory,
        file_name(manifest_partial)?,
        file_name(&manifest.manifest_path)?,
    )?;
    directory.sync_all()?;
    Ok(())
}

#[cfg(windows)]
fn write_output_pair(
    manifest: &RecoveryManifest,
    data: &[u8],
    output_partial: &Path,
    manifest_partial: &Path,
    output_identity: Option<&StorageIdentity>,
) -> Result<(), SafeWriterError> {
    let output_parent = manifest
        .output
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(manifest.output.clone()))?;
    let directory = open_verified_output_directory(output_parent)?;
    if let Some(identity) = output_identity {
        verify_opened_output_storage(&directory, identity)?;
    }

    let mut output_file = create_new_in_windows_directory(
        &directory,
        output_partial.file_name().ok_or_else(|| {
            SafeWriterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "output partial path has no file name",
            ))
        })?,
    )?;
    write_recovered_data(&mut output_file, data)?;
    output_file.sync_all()?;

    let mut manifest_file = create_new_in_windows_directory(
        &directory,
        manifest_partial.file_name().ok_or_else(|| {
            SafeWriterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "manifest partial path has no file name",
            ))
        })?,
    )?;
    write_manifest_contents(manifest, &mut manifest_file)?;
    manifest_file.sync_all()?;

    commit_no_replace_at_windows(
        &directory,
        &output_file,
        manifest.output.file_name().ok_or_else(|| {
            SafeWriterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "output path has no file name",
            ))
        })?,
    )?;
    commit_no_replace_at_windows(
        &directory,
        &manifest_file,
        manifest.manifest_path.file_name().ok_or_else(|| {
            SafeWriterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "manifest path has no file name",
            ))
        })?,
    )?;
    // Windows does not provide a documented directory-fsync equivalent, and
    // FlushFileBuffers on a directory handle commonly returns ACCESS_DENIED.
    // Both files were flushed before their handle-relative, no-replace rename.
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn write_output_pair(
    manifest: &RecoveryManifest,
    data: &[u8],
    output_partial: &Path,
    manifest_partial: &Path,
    _output_identity: Option<&StorageIdentity>,
) -> Result<(), SafeWriterError> {
    let mut output_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output_partial)?;
    write_recovered_data(&mut output_file, data)?;
    output_file.sync_all()?;

    let mut manifest_file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(manifest_partial)?;
    write_manifest_contents(manifest, &mut manifest_file)?;
    manifest_file.sync_all()?;

    commit_no_replace(output_partial, &manifest.output)?;
    commit_no_replace(manifest_partial, &manifest.manifest_path)?;
    Ok(())
}

pub fn validate_output_path(source: &Path, output: &Path) -> Result<(), SafeWriterError> {
    reject_output_path_components(output)?;
    if source == output {
        return Err(SafeWriterError::OutputEqualsSource);
    }

    if let (Ok(source_abs), Ok(output_abs)) =
        (std::fs::canonicalize(source), std::fs::canonicalize(output))
    {
        if source_abs == output_abs {
            return Err(SafeWriterError::OutputEqualsSource);
        }
    }

    let source_parent = canonical_parent(source)?;
    let output_parent = output
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(output.to_path_buf()))?;
    if !output_parent.exists() {
        return Err(SafeWriterError::MissingOutputParent(
            output_parent.to_path_buf(),
        ));
    }
    let output_parent = std::fs::canonicalize(output_parent)?;

    if output_parent == source_parent {
        return Err(SafeWriterError::OutputInsideSourceDirectory {
            source_parent,
            output: output.to_path_buf(),
        });
    }

    Ok(())
}

fn canonical_parent(path: &Path) -> Result<PathBuf, SafeWriterError> {
    let parent = path
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(path.to_path_buf()))?;
    Ok(std::fs::canonicalize(parent)?)
}

#[cfg(unix)]
fn reject_output_path_components(path: &Path) -> Result<(), SafeWriterError> {
    let parent = path
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(path.to_path_buf()))?;
    reject_symlink_components(parent)?;

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(
            SafeWriterError::OutputPathContainsSymlink(path.to_path_buf()),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(windows)]
fn reject_output_path_components(path: &Path) -> Result<(), SafeWriterError> {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    let parent = path
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(path.to_path_buf()))?;
    reject_windows_reparse_components(parent)?;

    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 => Err(
            SafeWriterError::OutputPathContainsSymlink(path.to_path_buf()),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(not(any(unix, windows)))]
fn reject_output_path_components(_path: &Path) -> Result<(), SafeWriterError> {
    Ok(())
}

fn validate_output_capacity(output: &Path, data_len: usize) -> Result<(), SafeWriterError> {
    let parent = output
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(output.to_path_buf()))?;
    let available_bytes = available_space(parent)?;
    validate_available_capacity(available_bytes, data_len)
}

fn validate_available_capacity(
    available_bytes: u64,
    data_len: usize,
) -> Result<(), SafeWriterError> {
    let required_bytes = required_output_capacity(data_len)?;
    if available_bytes < required_bytes {
        return Err(SafeWriterError::InsufficientFreeSpace {
            available_bytes,
            required_bytes,
        });
    }
    Ok(())
}

fn required_output_capacity(data_len: usize) -> Result<u64, SafeWriterError> {
    let data_bytes = u64::try_from(data_len).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "recovered data length does not fit in u64",
        )
    })?;
    let proportional_margin = data_bytes
        .checked_add(19)
        .and_then(|value| value.checked_div(20))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "output-space estimate overflow",
            )
        })?;
    data_bytes
        .checked_add(proportional_margin.max(MIN_FREE_SPACE_MARGIN_BYTES))
        .and_then(|value| value.checked_add(MANIFEST_SPACE_RESERVE_BYTES))
        .ok_or_else(|| {
            SafeWriterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "output-space estimate overflow",
            ))
        })
}

#[cfg(unix)]
fn available_space(path: &Path) -> Result<u64, SafeWriterError> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::os::unix::ffi::OsStrExt;

    let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output directory path contains a NUL byte",
        )
    })?;
    let mut stats = MaybeUninit::<libc::statvfs>::uninit();
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    let stats = unsafe { stats.assume_init() };
    let available = u128::from(stats.f_bavail) * u128::from(stats.f_frsize);
    Ok(available.min(u128::from(u64::MAX)) as u64)
}

#[cfg(windows)]
fn available_space(path: &Path) -> Result<u64, SafeWriterError> {
    use std::os::windows::ffi::OsStrExt;

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let mut available = 0u64;
    let result = unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if result == 0 {
        Err(SafeWriterError::Io(std::io::Error::last_os_error()))
    } else {
        Ok(available)
    }
}

#[cfg(not(any(unix, windows)))]
fn available_space(_path: &Path) -> Result<u64, SafeWriterError> {
    Err(SafeWriterError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "output free-space checks are unsupported on this platform",
    )))
}

fn manifest_path_for(output: &Path) -> PathBuf {
    let mut file_name = output
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "recovered".into());
    file_name.push(".manifest.txt");
    output.with_file_name(file_name)
}

fn partial_path_for(path: &Path) -> PathBuf {
    let mut file_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "recovered".into());
    file_name.push(".partial");
    path.with_file_name(file_name)
}

#[derive(Debug)]
struct ParsedRecoveryManifest {
    output: PathBuf,
    bytes_written: u64,
    sha256: String,
}

fn parse_recovery_manifest(path: &Path) -> Result<ParsedRecoveryManifest, SafeWriterError> {
    let contents = std::fs::read_to_string(path)?;
    let mut output = None;
    let mut bytes_written = None;
    let mut sha256 = None;
    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "output" => output = Some(PathBuf::from(value)),
            "bytes_written" => {
                bytes_written = Some(value.parse::<u64>().map_err(|_| {
                    SafeWriterError::OutputPairNeedsManualRecovery(format!(
                        "invalid bytes_written in {}",
                        path.display()
                    ))
                })?)
            }
            "sha256" => sha256 = Some(value.to_ascii_lowercase()),
            _ => {}
        }
    }
    let missing = || {
        SafeWriterError::OutputPairNeedsManualRecovery(format!(
            "incomplete manifest partial: {}",
            path.display()
        ))
    };
    let parsed = ParsedRecoveryManifest {
        output: output.ok_or_else(missing)?,
        bytes_written: bytes_written.ok_or_else(missing)?,
        sha256: sha256.ok_or_else(missing)?,
    };
    if parsed.sha256.len() != 64 || !parsed.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(SafeWriterError::OutputPairNeedsManualRecovery(format!(
            "invalid SHA-256 in {}",
            path.display()
        )));
    }
    Ok(parsed)
}

fn hash_file(path: &Path) -> Result<(u64, String), SafeWriterError> {
    use std::io::Read;

    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut total = 0u64;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total.checked_add(read as u64).ok_or_else(|| {
            SafeWriterError::OutputPairNeedsManualRecovery(
                "output size overflow while reconciling".to_string(),
            )
        })?;
        hasher.update(&buffer[..read]);
    }
    Ok((total, hasher.finalize_hex()))
}

/// Finish the only safe interrupted commit state: data is final, while its
/// already-synced manifest is still `.partial`.
///
/// Returns `true` when a manifest was reconciled. Any other incomplete or
/// inconsistent state fails closed and leaves every artifact untouched.
pub fn reconcile_recovery_output(output: impl AsRef<Path>) -> Result<bool, SafeWriterError> {
    let output = output.as_ref();
    reject_output_path_components(output)?;
    let manifest = manifest_path_for(output);
    let output_partial = partial_path_for(output);
    let manifest_partial = partial_path_for(&manifest);

    if manifest.exists() {
        if output.exists() && !output_partial.exists() && !manifest_partial.exists() {
            return Ok(false);
        }
        return Err(SafeWriterError::OutputPairNeedsManualRecovery(format!(
            "final manifest exists with unexpected partial or missing data for {}",
            output.display()
        )));
    }
    if !output.exists() {
        if output_partial.exists() || manifest_partial.exists() {
            return Err(SafeWriterError::OutputPairNeedsManualRecovery(format!(
                "uncommitted partial pair remains for {}",
                output.display()
            )));
        }
        return Ok(false);
    }
    if output_partial.exists() || !manifest_partial.exists() {
        return Err(SafeWriterError::OutputPairNeedsManualRecovery(format!(
            "committed data has no unique synced manifest partial for {}",
            output.display()
        )));
    }

    let parsed = parse_recovery_manifest(&manifest_partial)?;
    let expected_output = std::fs::canonicalize(output)?;
    let recorded_output = std::fs::canonicalize(&parsed.output).map_err(|error| {
        SafeWriterError::OutputPairNeedsManualRecovery(format!(
            "recorded output path cannot be resolved: {error}"
        ))
    })?;
    if recorded_output != expected_output {
        return Err(SafeWriterError::OutputPairNeedsManualRecovery(format!(
            "manifest output does not match committed data: {}",
            output.display()
        )));
    }
    let (actual_size, actual_hash) = hash_file(output)?;
    if actual_size != parsed.bytes_written || actual_hash != parsed.sha256 {
        return Err(SafeWriterError::OutputPairNeedsManualRecovery(format!(
            "committed data does not match its manifest partial: {}",
            output.display()
        )));
    }

    commit_reconciled_manifest(output, &manifest_partial, &manifest)?;
    Ok(true)
}

fn write_manifest_contents(
    manifest: &RecoveryManifest,
    file: &mut File,
) -> Result<(), SafeWriterError> {
    writeln!(file, "source={}", manifest.source.display())?;
    writeln!(file, "output={}", manifest.output.display())?;
    writeln!(file, "bytes_written={}", manifest.bytes_written)?;
    writeln!(file, "sha256={}", manifest.sha256)?;
    if let Some(record) = manifest.source_record {
        writeln!(file, "source_record={record}")?;
    }
    Ok(())
}

#[cfg(unix)]
fn file_name(path: &Path) -> Result<&std::ffi::OsStr, SafeWriterError> {
    path.file_name().ok_or_else(|| {
        SafeWriterError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("output path has no file name: {}", path.display()),
        ))
    })
}

#[cfg(unix)]
fn c_name(name: &std::ffi::OsStr) -> Result<std::ffi::CString, SafeWriterError> {
    use std::os::unix::ffi::OsStrExt;

    std::ffi::CString::new(name.as_bytes()).map_err(|_| {
        SafeWriterError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output file name contains a NUL byte",
        ))
    })
}

#[cfg(unix)]
fn reject_symlink_components(path: &Path) -> Result<(), SafeWriterError> {
    use std::path::Component;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::RootDir => current.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                current.pop();
            }
            Component::Normal(part) => {
                current.push(part);
                if std::fs::symlink_metadata(&current)?
                    .file_type()
                    .is_symlink()
                {
                    return Err(SafeWriterError::OutputPathContainsSymlink(current));
                }
            }
            Component::Prefix(_) => unreachable!("Unix paths do not have Windows prefixes"),
        }
    }
    Ok(())
}

#[cfg(unix)]
fn open_verified_output_directory(path: &Path) -> Result<File, SafeWriterError> {
    use std::os::fd::FromRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;

    reject_symlink_components(path)?;
    let canonical = std::fs::canonicalize(path)?;
    let expected = std::fs::metadata(&canonical)?;
    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| {
        SafeWriterError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "output directory path contains a NUL byte",
        ))
    })?;
    let fd = unsafe {
        libc::open(
            c_path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    let directory = unsafe { File::from_raw_fd(fd) };
    let actual = directory.metadata()?;
    if expected.dev() != actual.dev() || expected.ino() != actual.ino() {
        return Err(SafeWriterError::OutputDirectoryChanged(path.to_path_buf()));
    }
    Ok(directory)
}

#[cfg(target_os = "macos")]
fn macos_physical_stores_for_fd(
    fd: std::os::fd::RawFd,
) -> Result<BTreeSet<String>, SafeWriterError> {
    use std::ffi::CStr;
    use std::mem::MaybeUninit;

    let mut stats = MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(fd, stats.as_mut_ptr()) } != 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    let stats = unsafe { stats.assume_init() };
    let mount_source = unsafe { CStr::from_ptr(stats.f_mntfromname.as_ptr()) }
        .to_str()
        .map_err(|_| {
            SafeWriterError::StorageIdentityResolution(
                "opened file has a non-UTF-8 mount source".to_string(),
            )
        })?;
    macos_physical_stores_for_target(mount_source)
}

#[cfg(target_os = "macos")]
fn verify_opened_source_fd(
    fd: std::os::fd::RawFd,
    expected: &StorageIdentity,
) -> Result<(), SafeWriterError> {
    use std::mem::MaybeUninit;

    let expected_file =
        expected
            .file_object
            .ok_or_else(|| SafeWriterError::UnboundStorageIdentity {
                label: expected.label.clone(),
            })?;
    let mut stats = MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, stats.as_mut_ptr()) } != 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    let stats = unsafe { stats.assume_init() };
    let actual_device = stats.st_dev as u64;
    let actual_inode = stats.st_ino;
    if actual_device != expected_file.device || actual_inode != expected_file.inode {
        return Err(SafeWriterError::SourceFileIdentityChanged {
            expected_device: expected_file.device,
            expected_inode: expected_file.inode,
            actual_device,
            actual_inode,
        });
    }

    let actual_stores = macos_physical_stores_for_fd(fd)?;
    if actual_stores != expected.physical_stores {
        return Err(SafeWriterError::SourceStorageIdentityChanged {
            expected: expected.physical_stores.iter().cloned().collect(),
            actual: actual_stores.into_iter().collect(),
        });
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn verify_opened_output_storage(
    directory: &File,
    expected: &StorageIdentity,
) -> Result<(), SafeWriterError> {
    use std::os::fd::AsRawFd;

    #[cfg(debug_assertions)]
    if expected.provenance == StorageIdentityProvenance::DebugFixture {
        return Ok(());
    }
    if expected.provenance != StorageIdentityProvenance::MacOsResolved {
        return Err(SafeWriterError::UnboundStorageIdentity {
            label: expected.label.clone(),
        });
    }

    let actual = macos_physical_stores_for_fd(directory.as_raw_fd())?;
    if actual != expected.physical_stores {
        return Err(SafeWriterError::OutputStorageIdentityChanged {
            expected: expected.physical_stores.iter().cloned().collect(),
            actual: actual.into_iter().collect(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn verify_opened_source_handle(
    handle: windows_sys::Win32::Foundation::HANDLE,
    expected: &StorageIdentity,
) -> Result<(), SafeWriterError> {
    let expected_file =
        expected
            .file_object
            .ok_or_else(|| SafeWriterError::UnboundStorageIdentity {
                label: expected.label.clone(),
            })?;
    let actual_file = windows_file_object_identity(handle)?;
    if actual_file != expected_file {
        return Err(SafeWriterError::SourceFileIdentityChanged {
            expected_device: expected_file.volume_serial,
            expected_inode: expected_file.file_index,
            actual_device: actual_file.volume_serial,
            actual_inode: actual_file.file_index,
        });
    }
    let actual_stores = windows_physical_stores_for_handle(handle)?;
    if actual_stores != expected.physical_stores {
        return Err(SafeWriterError::SourceStorageIdentityChanged {
            expected: expected.physical_stores.iter().cloned().collect(),
            actual: actual_stores.into_iter().collect(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn verify_opened_output_storage(
    directory: &File,
    expected: &StorageIdentity,
) -> Result<(), SafeWriterError> {
    use std::os::windows::io::AsRawHandle;

    #[cfg(debug_assertions)]
    if expected.provenance == StorageIdentityProvenance::DebugFixture {
        return Ok(());
    }
    if expected.provenance != StorageIdentityProvenance::WindowsResolved {
        return Err(SafeWriterError::UnboundStorageIdentity {
            label: expected.label.clone(),
        });
    }
    let handle = directory.as_raw_handle() as _;
    if let Some(expected_file) = expected.file_object {
        if windows_file_object_identity(handle)? != expected_file {
            return Err(SafeWriterError::OutputDirectoryChanged(PathBuf::from(
                &expected.label,
            )));
        }
    }
    let actual = windows_physical_stores_for_handle(handle)?;
    if actual != expected.physical_stores {
        return Err(SafeWriterError::OutputStorageIdentityChanged {
            expected: expected.physical_stores.iter().cloned().collect(),
            actual: actual.into_iter().collect(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn reject_windows_reparse_components(path: &Path) -> Result<(), SafeWriterError> {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut current = PathBuf::new();
    for component in absolute.components() {
        current.push(component.as_os_str());
        if current.parent().is_none() {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&current)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(SafeWriterError::OutputPathContainsSymlink(current));
        }
    }
    Ok(())
}

#[cfg(windows)]
fn open_verified_output_directory(path: &Path) -> Result<File, SafeWriterError> {
    reject_windows_reparse_components(path)?;
    let directory = open_windows_path_handle(path, true)?;
    ensure_windows_directory(&directory, path)?;
    Ok(directory)
}

#[cfg(windows)]
fn windows_file_parent_matches_directory(
    file: &File,
    directory: &File,
) -> Result<(), SafeWriterError> {
    use std::os::windows::io::AsRawHandle;

    let file_path = windows_final_path_for_handle(file.as_raw_handle() as _)?;
    let parent = file_path.parent().ok_or_else(|| {
        SafeWriterError::StorageIdentityResolution(format!(
            "opened output file has no parent: {}",
            file_path.display()
        ))
    })?;
    let parent_handle = open_windows_path_handle(parent, true)?;
    let expected = windows_file_object_identity(directory.as_raw_handle() as _)?;
    let actual = windows_file_object_identity(parent_handle.as_raw_handle() as _)?;
    if actual != expected {
        return Err(SafeWriterError::OutputDirectoryChanged(
            parent.to_path_buf(),
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn create_new_in_windows_directory(
    directory: &File,
    name: &std::ffi::OsStr,
) -> Result<File, SafeWriterError> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Win32::Foundation::{GENERIC_WRITE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CREATE_NEW, CreateFileW, DELETE, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    };

    let directory_path = windows_final_path_for_handle(directory.as_raw_handle() as _)?;
    let path = directory_path.join(name);
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_WRITE | DELETE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    let file = unsafe { File::from_raw_handle(handle as _) };
    ensure_windows_regular_file(&file, &path)?;
    windows_file_parent_matches_directory(&file, directory)?;
    Ok(file)
}

#[cfg(windows)]
fn open_existing_in_windows_directory_for_rename(
    directory: &File,
    name: &std::ffi::OsStr,
) -> Result<File, SafeWriterError> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use windows_sys::Win32::Foundation::{GENERIC_READ, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, DELETE, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OPEN_REPARSE_POINT,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    let path = windows_final_path_for_handle(directory.as_raw_handle() as _)?.join(name);
    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ | DELETE,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    let file = unsafe { File::from_raw_handle(handle as _) };
    ensure_windows_regular_file(&file, &path)?;
    windows_file_parent_matches_directory(&file, directory)?;
    Ok(file)
}

#[cfg(windows)]
fn commit_no_replace_at_windows(
    directory: &File,
    file: &File,
    final_name: &std::ffi::OsStr,
) -> Result<(), SafeWriterError> {
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_RENAME_INFO, FileRenameInfo, SetFileInformationByHandle,
    };

    let final_path =
        windows_final_path_for_handle(directory.as_raw_handle() as _)?.join(final_name);
    let name = final_path.as_os_str().encode_wide().collect::<Vec<_>>();
    let name_bytes = name
        .len()
        .checked_mul(std::mem::size_of::<u16>())
        .ok_or_else(|| {
            SafeWriterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "output file name is too long",
            ))
        })?;
    // Keep the structure's trailing WCHAR and append the entire relative
    // name. The small over-allocation is intentional: Windows validates the
    // information buffer size more strictly than the nominal flexible-array
    // calculation on some filesystem drivers.
    let buffer_size = std::mem::size_of::<FILE_RENAME_INFO>()
        .checked_add(name_bytes)
        .ok_or_else(|| {
            SafeWriterError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "rename buffer size overflow",
            ))
        })?;
    let mut buffer = vec![0u8; buffer_size];
    let information = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        (*information).Anonymous.ReplaceIfExists = false;
        (*information).RootDirectory = std::ptr::null_mut();
        (*information).FileNameLength = name_bytes as u32;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            (*information).FileName.as_mut_ptr(),
            name.len(),
        );
    }
    let ok = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as _,
            FileRenameInfo,
            buffer.as_ptr().cast(),
            buffer.len() as u32,
        )
    };
    if ok == 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    windows_file_parent_matches_directory(file, directory)?;
    Ok(())
}

#[cfg(windows)]
fn commit_reconciled_manifest(
    output: &Path,
    manifest_partial: &Path,
    manifest: &Path,
) -> Result<(), SafeWriterError> {
    let parent = output
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(output.to_path_buf()))?;
    let directory = open_verified_output_directory(parent)?;
    let partial_name = manifest_partial.file_name().ok_or_else(|| {
        SafeWriterError::OutputPairNeedsManualRecovery(
            "manifest partial has no file name".to_string(),
        )
    })?;
    let final_name = manifest.file_name().ok_or_else(|| {
        SafeWriterError::OutputPairNeedsManualRecovery("manifest path has no file name".to_string())
    })?;
    let partial = open_existing_in_windows_directory_for_rename(&directory, partial_name)?;
    commit_no_replace_at_windows(&directory, &partial, final_name)
}

#[cfg(unix)]
fn commit_reconciled_manifest(
    output: &Path,
    manifest_partial: &Path,
    manifest: &Path,
) -> Result<(), SafeWriterError> {
    let parent = output
        .parent()
        .ok_or_else(|| SafeWriterError::MissingOutputParent(output.to_path_buf()))?;
    let directory = open_verified_output_directory(parent)?;
    commit_no_replace_at(
        &directory,
        file_name(manifest_partial)?,
        file_name(manifest)?,
    )?;
    directory.sync_all()?;
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn commit_reconciled_manifest(
    _output: &Path,
    manifest_partial: &Path,
    manifest: &Path,
) -> Result<(), SafeWriterError> {
    commit_no_replace(manifest_partial, manifest)
}

#[cfg(unix)]
fn create_new_at(directory: &File, name: &std::ffi::OsStr) -> Result<File, SafeWriterError> {
    use std::os::fd::{AsRawFd, FromRawFd};

    let name = c_name(name)?;
    let fd = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            0o600 as libc::c_uint,
        )
    };
    if fd < 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(target_os = "macos")]
fn commit_no_replace_at(
    directory: &File,
    partial: &std::ffi::OsStr,
    final_name: &std::ffi::OsStr,
) -> Result<(), SafeWriterError> {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;

    let from: CString = c_name(partial)?;
    let to: CString = c_name(final_name)?;
    let result = unsafe {
        libc::renameatx_np(
            directory.as_raw_fd(),
            from.as_ptr(),
            directory.as_raw_fd(),
            to.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(SafeWriterError::Io(std::io::Error::last_os_error()))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn commit_no_replace_at(
    directory: &File,
    partial: &std::ffi::OsStr,
    final_name: &std::ffi::OsStr,
) -> Result<(), SafeWriterError> {
    use std::os::fd::AsRawFd;

    let from = c_name(partial)?;
    let to = c_name(final_name)?;
    let link_result = unsafe {
        libc::linkat(
            directory.as_raw_fd(),
            from.as_ptr(),
            directory.as_raw_fd(),
            to.as_ptr(),
            0,
        )
    };
    if link_result != 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    if unsafe { libc::unlinkat(directory.as_raw_fd(), from.as_ptr(), 0) } != 0 {
        return Err(SafeWriterError::Io(std::io::Error::last_os_error()));
    }
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn commit_no_replace(partial: &Path, final_path: &Path) -> Result<(), SafeWriterError> {
    std::fs::rename(partial, final_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temp_dir_name() -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/test-safe-writer")
            .join(format!("dr_safe_writer_{}_{id}", std::process::id()))
    }

    #[test]
    fn writes_file_and_manifest() {
        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let source = source_dir.join("image.bin");
        std::fs::write(&source, b"source").unwrap();
        let output = output_dir.join("report.txt");

        let manifest =
            write_recovered_file(&source, &output, b"hello recovered data", Some(42)).unwrap();

        assert_eq!(std::fs::read(&output).unwrap(), b"hello recovered data");
        assert!(manifest.manifest_path.exists());
        assert!(!partial_path_for(&output).exists());
        assert!(!partial_path_for(&manifest.manifest_path).exists());
        assert_eq!(manifest.bytes_written, 20);
        assert_eq!(
            manifest.sha256,
            "a2ac8f445971c0ffa9deda2237db94ccb62058c75de0f7494f6cbcf76a32231c"
        );

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn never_overwrites_existing_output() {
        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let source = source_dir.join("image.bin");
        let output = output_dir.join("report.txt");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&output, b"existing").unwrap();

        assert!(write_recovered_file(&source, &output, b"replacement", None).is_err());
        assert_eq!(std::fs::read(&output).unwrap(), b"existing");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn disk_full_injection_leaves_only_a_partial_data_file() {
        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let source = source_dir.join("image.bin");
        let output = output_dir.join("report.txt");
        std::fs::write(&source, b"source").unwrap();

        TEST_DISK_FULL_AFTER.with(|limit| limit.set(Some(5)));
        let result = write_recovered_file(&source, &output, b"0123456789", None);
        TEST_DISK_FULL_AFTER.with(|limit| limit.set(None));

        assert!(matches!(
            result,
            Err(SafeWriterError::Io(error)) if error.to_string().contains("injected disk-full")
        ));
        assert!(!output.exists());
        assert_eq!(std::fs::read(partial_path_for(&output)).unwrap(), b"01234");
        assert!(!manifest_path_for(&output).exists());
        assert!(!partial_path_for(&manifest_path_for(&output)).exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reconciles_committed_data_with_its_synced_manifest_partial() {
        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let source = source_dir.join("image.bin");
        let output = output_dir.join("report.txt");
        let data = b"committed recovered bytes";
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&output, data).unwrap();
        let manifest_path = manifest_path_for(&output);
        let manifest_partial = partial_path_for(&manifest_path);
        let manifest = RecoveryManifest {
            source,
            output: output.clone(),
            manifest_path: manifest_path.clone(),
            bytes_written: data.len(),
            sha256: sha256_hex(data),
            source_record: Some(7),
        };
        let mut partial = File::create(&manifest_partial).unwrap();
        write_manifest_contents(&manifest, &mut partial).unwrap();
        partial.sync_all().unwrap();
        drop(partial);

        assert!(reconcile_recovery_output(&output).unwrap());
        assert!(manifest_path.exists());
        assert!(!manifest_partial.exists());
        assert!(!reconcile_recovery_output(&output).unwrap());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_to_reconcile_mismatched_committed_data() {
        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let source = source_dir.join("image.bin");
        let output = output_dir.join("report.txt");
        std::fs::write(&source, b"source").unwrap();
        std::fs::write(&output, b"corrupted bytes").unwrap();
        let manifest_path = manifest_path_for(&output);
        let manifest_partial = partial_path_for(&manifest_path);
        let manifest = RecoveryManifest {
            source,
            output: output.clone(),
            manifest_path: manifest_path.clone(),
            bytes_written: 8,
            sha256: sha256_hex(b"expected"),
            source_record: None,
        };
        let mut partial = File::create(&manifest_partial).unwrap();
        write_manifest_contents(&manifest, &mut partial).unwrap();
        partial.sync_all().unwrap();
        drop(partial);

        assert!(matches!(
            reconcile_recovery_output(&output),
            Err(SafeWriterError::OutputPairNeedsManualRecovery(_))
        ));
        assert!(output.exists());
        assert!(manifest_partial.exists());
        assert!(!manifest_path.exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_output_parent() {
        use std::os::unix::fs::symlink;

        let root = temp_dir_name();
        let source_dir = root.join("source");
        let real_output_dir = root.join("real-output");
        let linked_output_dir = root.join("linked-output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&real_output_dir).unwrap();
        symlink(&real_output_dir, &linked_output_dir).unwrap();
        let source = source_dir.join("image.bin");
        let output = linked_output_dir.join("report.txt");
        std::fs::write(&source, b"source").unwrap();

        let result = write_recovered_file(&source, &output, b"must not be written", None);
        match result {
            Err(SafeWriterError::OutputPathContainsSymlink(path)) => {
                assert_eq!(path.file_name(), linked_output_dir.file_name());
            }
            other => panic!("expected symlink rejection, got {other:?}"),
        }
        assert!(!real_output_dir.join("report.txt").exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_intermediate_output_parent() {
        use std::os::unix::fs::symlink;

        let root = temp_dir_name();
        let source_dir = root.join("source");
        let real_output_dir = root.join("real-output");
        let linked_parent = root.join("nested").join("linked-output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(real_output_dir.join("child")).unwrap();
        std::fs::create_dir_all(linked_parent.parent().unwrap()).unwrap();
        symlink(&real_output_dir, &linked_parent).unwrap();
        let source = source_dir.join("image.bin");
        let output = linked_parent.join("child").join("report.txt");
        std::fs::write(&source, b"source").unwrap();

        let result = write_recovered_file(&source, &output, b"must not be written", None);
        match result {
            Err(SafeWriterError::OutputPathContainsSymlink(path)) => {
                assert_eq!(path.file_name(), linked_parent.file_name());
            }
            other => panic!("expected symlink rejection, got {other:?}"),
        }
        assert!(!real_output_dir.join("child").join("report.txt").exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlinked_output_file() {
        use std::os::unix::fs::symlink;

        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        let target = output_dir.join("target.txt");
        let output = output_dir.join("report.txt");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        std::fs::write(&target, b"original target").unwrap();
        symlink(&target, &output).unwrap();
        let source = source_dir.join("image.bin");
        std::fs::write(&source, b"source").unwrap();

        let result = write_recovered_file(&source, &output, b"must not be written", None);
        match result {
            Err(SafeWriterError::OutputPathContainsSymlink(path)) => {
                assert_eq!(path, output);
            }
            other => panic!("expected symlink rejection, got {other:?}"),
        }
        assert_eq!(std::fs::read(&target).unwrap(), b"original target");

        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn directory_handle_stays_bound_after_path_replacement() {
        use std::ffi::OsStr;

        let root = temp_dir_name();
        let output_dir = root.join("output");
        let moved_dir = root.join("moved-output");
        std::fs::create_dir_all(&output_dir).unwrap();
        let directory = open_verified_output_directory(&output_dir).unwrap();

        std::fs::rename(&output_dir, &moved_dir).unwrap();
        std::fs::create_dir(&output_dir).unwrap();

        let mut file = create_new_at(&directory, OsStr::new("bound.txt.partial")).unwrap();
        file.write_all(b"bound to original directory").unwrap();
        file.sync_all().unwrap();
        commit_no_replace_at(
            &directory,
            OsStr::new("bound.txt.partial"),
            OsStr::new("bound.txt"),
        )
        .unwrap();
        directory.sync_all().unwrap();
        drop(file);
        drop(directory);

        assert_eq!(
            std::fs::read(moved_dir.join("bound.txt")).unwrap(),
            b"bound to original directory"
        );
        assert!(!output_dir.join("bound.txt").exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn output_capacity_includes_margin_and_manifest_reserve() {
        assert_eq!(
            required_output_capacity(1024).unwrap(),
            1024 + MIN_FREE_SPACE_MARGIN_BYTES + MANIFEST_SPACE_RESERVE_BYTES
        );
        assert_eq!(
            required_output_capacity(2_000_000_000).unwrap(),
            2_100_000_000 + MANIFEST_SPACE_RESERVE_BYTES
        );
    }

    #[test]
    fn rejects_insufficient_output_capacity() {
        assert!(matches!(
            validate_available_capacity(MIN_FREE_SPACE_MARGIN_BYTES, 1024),
            Err(SafeWriterError::InsufficientFreeSpace {
                available_bytes,
                required_bytes,
            }) if available_bytes == MIN_FREE_SPACE_MARGIN_BYTES
                && required_bytes == 1024 + MIN_FREE_SPACE_MARGIN_BYTES
                    + MANIFEST_SPACE_RESERVE_BYTES
        ));
    }

    #[test]
    fn rejects_output_in_source_directory() {
        let root = temp_dir_name();
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("image.bin");
        std::fs::write(&source, b"source").unwrap();
        let output = root.join("report.txt");

        assert!(matches!(
            validate_output_path(&source, &output),
            Err(SafeWriterError::OutputInsideSourceDirectory { .. })
        ));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_unknown_source_identity() {
        let source = StorageIdentity::unknown("source image");
        let output = StorageIdentity::from_physical_stores("output", ["disk2"]);

        assert!(matches!(
            validate_storage_isolation(&source, &output),
            Err(SafeWriterError::UnknownStorageIdentity { label })
                if label == "source image"
        ));
    }

    #[test]
    fn rejects_unknown_output_identity() {
        let source = StorageIdentity::from_physical_stores("source", ["disk4"]);
        let output = StorageIdentity::unknown("output directory");

        assert!(matches!(
            validate_storage_isolation(&source, &output),
            Err(SafeWriterError::UnknownStorageIdentity { label })
                if label == "output directory"
        ));
    }

    #[test]
    fn rejects_shared_physical_store_across_distinct_volumes() {
        let source =
            StorageIdentity::from_physical_stores("APFS source volume", ["disk0", "disk1"]);
        let output =
            StorageIdentity::from_physical_stores("APFS output volume", ["disk1", "disk2"]);

        assert!(matches!(
            validate_storage_isolation(&source, &output),
            Err(SafeWriterError::SharedPhysicalStorage { stores })
                if stores == vec!["disk1"]
        ));
    }

    #[test]
    fn accepts_disjoint_complete_storage_identities() {
        let source = StorageIdentity::from_physical_stores("source", ["disk4"]);
        let output = StorageIdentity::from_physical_stores("output", ["disk2"]);

        validate_storage_isolation(&source, &output).unwrap();
    }

    #[test]
    fn verified_writer_checks_storage_before_creating_output() {
        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let source = source_dir.join("image.bin");
        let output = output_dir.join("report.txt");
        std::fs::write(&source, b"source").unwrap();
        let source_identity = StorageIdentity::debug_fixture("source", ["disk4"]);
        let output_identity = StorageIdentity::debug_fixture("output", ["disk4"]);
        let source_file = FileBlockSource::open(&source).unwrap();
        let verified_source = verify_open_source_storage(&source_file, &source_identity).unwrap();

        let result = write_recovered_file_with_storage_identities(
            &source,
            &output,
            b"must not be written",
            None,
            &verified_source,
            &output_identity,
        );

        assert!(matches!(
            result,
            Err(SafeWriterError::SharedPhysicalStorage { .. })
        ));
        assert!(!output.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn verified_writer_rejects_caller_asserted_unbound_identities() {
        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let source = source_dir.join("image.bin");
        let output = output_dir.join("report.txt");
        std::fs::write(&source, b"source").unwrap();
        let source_identity = StorageIdentity::from_physical_stores("source", ["disk4"]);
        let source_file = FileBlockSource::open(&source).unwrap();
        let result = verify_open_source_storage(&source_file, &source_identity);

        assert!(matches!(
            result,
            Err(SafeWriterError::UnboundStorageIdentity { label }) if label == "source"
        ));
        assert!(!output.exists());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_storage_identity_normalizes_disk_slices() {
        assert_eq!(
            macos_whole_disk_identifier("/dev/disk12s3"),
            Some("disk12".to_string())
        );
        assert_eq!(
            macos_whole_disk_identifier("disk2"),
            Some("disk2".to_string())
        );
        assert_eq!(macos_whole_disk_identifier("not-a-device"), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_storage_identity_keeps_every_apfs_physical_store() {
        let mut first = plist::Dictionary::new();
        first.insert(
            "DeviceIdentifier".to_string(),
            plist::Value::String("disk0s2".to_string()),
        );
        let mut second = plist::Dictionary::new();
        second.insert(
            "APFSPhysicalStore".to_string(),
            plist::Value::String("/dev/disk2s4".to_string()),
        );
        let mut info = plist::Dictionary::new();
        info.insert(
            "APFSPhysicalStores".to_string(),
            plist::Value::Array(vec![
                plist::Value::Dictionary(first),
                plist::Value::Dictionary(second),
            ]),
        );

        assert_eq!(
            macos_physical_stores_from_plist(&plist::Value::Dictionary(info)),
            BTreeSet::from(["disk0".to_string(), "disk2".to_string()])
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_storage_identity_parses_posix_df_device() {
        let output = "Filesystem 512-blocks Used Available Capacity Mounted on\n/dev/disk3s5s1 1000 500 500 50% /System/Volumes/Data\n";
        assert_eq!(
            macos_device_from_df_output(output).unwrap(),
            "/dev/disk3s5s1"
        );
        assert!(macos_device_from_df_output("Filesystem Mounted on\nmap auto_home\n").is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn opened_directory_matches_resolved_physical_storage() {
        let root = temp_dir_name();
        std::fs::create_dir_all(&root).unwrap();
        let stores = macos_physical_stores_for_path(&root).unwrap();
        let identity = StorageIdentity::macos_resolved(root.display().to_string(), stores, None);
        let directory = open_verified_output_directory(&root).unwrap();

        verify_opened_output_storage(&directory, &identity).unwrap();
        let changed = StorageIdentity::macos_resolved(
            root.display().to_string(),
            ["definitely-not-the-opened-store"],
            None,
        );
        assert!(matches!(
            verify_opened_output_storage(&directory, &changed),
            Err(SafeWriterError::OutputStorageIdentityChanged { .. })
        ));

        drop(directory);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn opened_source_must_match_preflight_file_identity() {
        use std::os::unix::fs::MetadataExt;

        let root = temp_dir_name();
        std::fs::create_dir_all(&root).unwrap();
        let source_path = root.join("source.bin");
        let original_path = root.join("original.bin");
        std::fs::write(&source_path, b"original fixture").unwrap();
        let metadata = std::fs::metadata(&source_path).unwrap();
        let stores = macos_physical_stores_for_path(&source_path).unwrap();
        let identity = StorageIdentity::macos_resolved(
            source_path.display().to_string(),
            stores,
            Some(FileObjectIdentity {
                device: metadata.dev(),
                inode: metadata.ino(),
            }),
        );

        let original = FileBlockSource::open(&source_path).unwrap();
        verify_open_source_storage(&original, &identity).unwrap();
        drop(original);

        std::fs::rename(&source_path, &original_path).unwrap();
        std::fs::write(&source_path, b"replacement fixture").unwrap();
        let replacement = FileBlockSource::open(&source_path).unwrap();
        assert!(matches!(
            verify_open_source_storage(&replacement, &identity),
            Err(SafeWriterError::SourceFileIdentityChanged { .. })
        ));

        drop(replacement);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn image_identity_rejects_raw_path_before_platform_lookup() {
        let result = resolve_image_storage_identities(
            Path::new("/dev/disk999"),
            Path::new("/tmp/polik-output/recovered.bin"),
        );
        assert!(matches!(
            result,
            Err(SafeWriterError::StorageIdentityResolution(message))
                if message.contains("RAW_DEVICE_UNSUPPORTED")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_image_identity_rejects_raw_path_before_platform_lookup() {
        let result = resolve_image_storage_identities(
            Path::new(r"\\.\PhysicalDrive999"),
            std::env::temp_dir()
                .join("polik-output")
                .join("recovered.bin"),
        );
        assert!(matches!(
            result,
            Err(SafeWriterError::StorageIdentityResolution(message))
                if message.contains("RAW_DEVICE_UNSUPPORTED")
        ));
    }

    #[cfg(windows)]
    #[test]
    fn windows_resolver_and_writer_bind_real_file_handles() {
        let source_root = temp_dir_name();
        let output_root = std::env::temp_dir().join(format!(
            "dr_safe_writer_windows_{}_{}",
            std::process::id(),
            NEXT_WINDOWS_TEST_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&source_root).unwrap();
        std::fs::create_dir_all(&output_root).unwrap();
        let source_path = source_root.join("source.img");
        let output_path = output_root.join("recovered.bin");
        std::fs::write(&source_path, b"ordinary image fixture").unwrap();

        match resolve_image_storage_identities(&source_path, &output_path) {
            Ok((source_identity, output_identity)) => {
                assert_eq!(
                    source_identity.provenance,
                    StorageIdentityProvenance::WindowsResolved
                );
                assert_eq!(
                    output_identity.provenance,
                    StorageIdentityProvenance::WindowsResolved
                );
                let source = FileBlockSource::open(&source_path).unwrap();
                let verified = verify_open_source_storage(&source, &source_identity).unwrap();
                write_recovered_file_with_storage_identities(
                    &source_path,
                    &output_path,
                    b"verified cross-volume output",
                    None,
                    &verified,
                    &output_identity,
                )
                .unwrap();
                assert_eq!(
                    std::fs::read(&output_path).unwrap(),
                    b"verified cross-volume output"
                );
            }
            Err(SafeWriterError::SharedPhysicalStorage { .. }) => {
                // A single-disk CI host cannot exercise a successful isolated
                // write, but must still prove that the resolver fails closed.
                assert!(!output_path.exists());
            }
            Err(other) => panic!("unexpected Windows resolver result: {other}"),
        }

        std::fs::remove_dir_all(source_root).unwrap();
        std::fs::remove_dir_all(output_root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn windows_resolver_rejects_distinct_paths_on_the_same_physical_disk() {
        let root = temp_dir_name();
        let source_dir = root.join("source");
        let output_dir = root.join("output");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let source = source_dir.join("source.img");
        let output = output_dir.join("recovered.bin");
        std::fs::write(&source, b"same-store fixture").unwrap();

        assert!(matches!(
            resolve_image_storage_identities(&source, &output),
            Err(SafeWriterError::SharedPhysicalStorage { stores }) if !stores.is_empty()
        ));
        assert!(!output.exists());

        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    static NEXT_WINDOWS_TEST_ID: AtomicU64 = AtomicU64::new(0);
}
