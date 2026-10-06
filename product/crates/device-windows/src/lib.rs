#[cfg(target_os = "windows")]
mod imp {
    use data_recovery_core::{BlockSource, BusType, PhysicalDevice, ReadError, ReadResult};
    use std::fs::File;
    use std::os::windows::fs::FileExt;
    use std::os::windows::io::FromRawHandle;
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::Foundation::GENERIC_READ;

    #[derive(Debug)]
    pub enum WindowsDeviceError {
        Io(std::io::Error),
        Read(ReadError),
        Win32 { function: &'static str, code: u32 },
        InvalidPath(String),
    }

    impl std::fmt::Display for WindowsDeviceError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Io(err) => write!(f, "I/O 錯誤: {err}"),
                Self::Read(err) => write!(f, "讀取錯誤: {err}"),
                Self::Win32 { function, code } => {
                    write!(f, "Win32 函數 {function} 失敗，錯誤碼: {code}")
                }
                Self::InvalidPath(path) => write!(f, "無效的裝置路徑: {path}"),
            }
        }
    }

    impl std::error::Error for WindowsDeviceError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self {
                Self::Io(err) => Some(err),
                Self::Read(err) => Some(err),
                _ => None,
            }
        }
    }

    impl From<std::io::Error> for WindowsDeviceError {
        fn from(value: std::io::Error) -> Self {
            Self::Io(value)
        }
    }

    impl From<ReadError> for WindowsDeviceError {
        fn from(value: ReadError) -> Self {
            Self::Read(value)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WindowsDiskInfo {
        pub device: PhysicalDevice,
        pub device_node: String,
        pub content: Option<String>,
        pub mount_point: Option<String>,
        pub whole_disk: bool,
        pub writable_media: Option<bool>,
    }

    fn to_wchar(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn list_whole_disks() -> Result<Vec<WindowsDiskInfo>, WindowsDeviceError> {
        let mut list = Vec::new();

        // 1. 枚舉實體硬碟 (PhysicalDrive0 - PhysicalDrive15)
        for i in 0..16 {
            let identifier = format!("PhysicalDrive{}", i);
            if let Ok(info) = disk_info(&identifier) {
                list.push(info);
            }
        }

        // 2. 枚舉邏輯磁碟區 (C:, D:, E: ...)
        unsafe {
            let mut buffer = [0u16; 512];
            let len = windows_sys::Win32::Storage::FileSystem::GetLogicalDriveStringsW(
                buffer.len() as u32,
                buffer.as_mut_ptr(),
            );
            if len > 0 && len < buffer.len() as u32 {
                let mut start = 0;
                while start < len as usize {
                    let slice = &buffer[start..];
                    let zero_pos = slice.iter().position(|&x| x == 0).unwrap_or(0);
                    if zero_pos == 0 {
                        break;
                    }
                    let drive_str = String::from_utf16_lossy(&slice[..zero_pos]);
                    start += zero_pos + 1;

                    // drive_str 通常是 "C:\" 格式
                    let clean_letter = drive_str.trim_end_matches('\\'); // 變為 "C:"
                    if let Ok(info) = disk_info(clean_letter) {
                        list.push(info);
                    }
                }
            }
        }

        Ok(list)
    }

    pub fn disk_info(identifier: &str) -> Result<WindowsDiskInfo, WindowsDeviceError> {
        let device_node = if identifier.starts_with(r"\\.\") {
            identifier.to_string()
        } else if identifier.len() == 1 || (identifier.len() == 2 && identifier.ends_with(':')) {
            let letter = identifier.chars().next().unwrap().to_ascii_uppercase();
            format!(r"\\.\{}:", letter)
        } else if identifier.starts_with("PhysicalDrive") {
            format!(r"\\.\{}", identifier)
        } else {
            return Err(WindowsDeviceError::InvalidPath(identifier.to_string()));
        };

        let path_w = to_wchar(&device_node);

        unsafe {
            // 先以唯讀與共享讀寫模式開啟
            let handle = windows_sys::Win32::Storage::FileSystem::CreateFileW(
                path_w.as_ptr(),
                GENERIC_READ,
                windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                    | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
                std::ptr::null(),
                windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING,
                windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            );

            if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                // 如果權限不足，嘗試以 0 (查詢權限) 開啟以獲取基本屬性
                let query_handle = windows_sys::Win32::Storage::FileSystem::CreateFileW(
                    path_w.as_ptr(),
                    0,
                    windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                        | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
                    std::ptr::null(),
                    windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING,
                    windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
                    std::ptr::null_mut(),
                );

                if query_handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                    return Err(WindowsDeviceError::Win32 {
                        function: "CreateFileW",
                        code: windows_sys::Win32::Foundation::GetLastError(),
                    });
                }

                let info = parse_disk_info(identifier, &device_node, query_handle);
                windows_sys::Win32::Foundation::CloseHandle(query_handle);
                return info;
            }

            let info = parse_disk_info(identifier, &device_node, handle);
            windows_sys::Win32::Foundation::CloseHandle(handle);
            info
        }
    }

    unsafe fn parse_disk_info(
        identifier: &str,
        device_node: &str,
        handle: windows_sys::Win32::Foundation::HANDLE,
    ) -> Result<WindowsDiskInfo, WindowsDeviceError> {
        use windows_sys::Win32::System::IO::DeviceIoControl;
        use windows_sys::Win32::System::Ioctl::{
            DISK_GEOMETRY_EX, IOCTL_DISK_GET_DRIVE_GEOMETRY_EX, IOCTL_DISK_GET_LENGTH_INFO,
            IOCTL_STORAGE_GET_DEVICE_NUMBER, STORAGE_DEVICE_NUMBER,
        };

        let whole_disk = device_node.contains("PhysicalDrive");
        let mut size_bytes = 0u64;
        let mut sector_size = 512u32;
        let mut is_removable = false;
        let mut is_system_disk = false;
        let mut bus_type = BusType::Unknown;

        // 1. 嘗試查詢磁碟幾何
        let mut geom: DISK_GEOMETRY_EX = unsafe { std::mem::zeroed() };
        let mut bytes_ret = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_DISK_GET_DRIVE_GEOMETRY_EX,
                std::ptr::null(),
                0,
                &mut geom as *mut _ as *mut _,
                std::mem::size_of::<DISK_GEOMETRY_EX>() as u32,
                &mut bytes_ret,
                std::ptr::null_mut(),
            )
        };

        if ok != 0 {
            size_bytes = geom.DiskSize as u64;
            sector_size = geom.Geometry.BytesPerSector;
        } else {
            // 如果是分割區，可能不支持 GET_DRIVE_GEOMETRY_EX，改用 IOCTL_DISK_GET_LENGTH_INFO
            let mut length_info = 0u64;
            let mut bytes_ret_length = 0u32;
            let ok_length = unsafe {
                DeviceIoControl(
                    handle,
                    IOCTL_DISK_GET_LENGTH_INFO,
                    std::ptr::null(),
                    0,
                    &mut length_info as *mut _ as *mut _,
                    std::mem::size_of::<u64>() as u32,
                    &mut bytes_ret_length,
                    std::ptr::null_mut(),
                )
            };
            if ok_length != 0 {
                size_bytes = length_info;
            }
        }

        // 2. 獲取設備號碼及類型以辨識匯流排
        let mut dev_num: STORAGE_DEVICE_NUMBER = unsafe { std::mem::zeroed() };
        let mut bytes_ret_num = 0u32;
        let ok_num = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_GET_DEVICE_NUMBER,
                std::ptr::null(),
                0,
                &mut dev_num as *mut _ as *mut _,
                std::mem::size_of::<STORAGE_DEVICE_NUMBER>() as u32,
                &mut bytes_ret_num,
                std::ptr::null_mut(),
            )
        };

        if ok_num != 0 && dev_num.DeviceType == 7 {
            bus_type = BusType::Sata;
        }

        // 3. 處理磁碟機代號 (例如 C:, D:)
        let mut mount_point = None;
        if !whole_disk {
            let letter = identifier.chars().next().unwrap().to_ascii_uppercase();
            let root_path = format!("{}:\\", letter);
            mount_point = Some(root_path.clone());

            let root_path_w = to_wchar(&root_path);
            let drive_type = unsafe {
                windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root_path_w.as_ptr())
            };

            match drive_type {
                2 => {
                    is_removable = true;
                    bus_type = BusType::Usb;
                }
                3 => {
                    is_removable = false;
                    bus_type = BusType::Sata;
                }
                _ => {}
            }

            if letter == 'C' {
                is_system_disk = true;
            }

            // 如果上面還是沒讀到 size_bytes，呼叫 GetDiskFreeSpaceExW 作為 fallback
            if size_bytes == 0 {
                let mut free_val = 0u64;
                let mut total_val = 0u64;
                let mut free_total_val = 0u64;
                let ok_free = unsafe {
                    windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
                        root_path_w.as_ptr(),
                        &mut free_val,
                        &mut total_val,
                        &mut free_total_val,
                    )
                };
                if ok_free != 0 {
                    size_bytes = total_val;
                }
            }
        } else if identifier == "PhysicalDrive0" {
            is_system_disk = true;
        }

        let display_name = if whole_disk {
            format!(
                "實體磁碟 {}",
                identifier
                    .strip_prefix("PhysicalDrive")
                    .unwrap_or(identifier)
            )
        } else {
            format!("本地磁碟 ({})", identifier)
        };

        Ok(WindowsDiskInfo {
            device: PhysicalDevice {
                id: identifier.to_string(),
                display_name,
                platform_path: device_node.to_string(),
                size_bytes,
                logical_sector_size: sector_size,
                physical_sector_size: sector_size,
                is_removable,
                is_system_disk,
                bus_type,
                trim_supported: None,
            },
            device_node: device_node.to_string(),
            content: if whole_disk {
                Some("PartitionTable: MBR/GPT".to_string())
            } else {
                None
            },
            mount_point,
            whole_disk,
            writable_media: Some(true),
        })
    }

    pub struct WindowsRawDiskSource {
        device: PhysicalDevice,
        file: File,
        raw_path: PathBuf,
    }

    impl WindowsRawDiskSource {
        pub fn open_identifier(identifier: &str) -> Result<Self, WindowsDeviceError> {
            let info = disk_info(identifier)?;
            Self::open_with_device(PathBuf::from(&info.device_node), info.device)
        }

        pub fn open_path(path: impl AsRef<Path>) -> Result<Self, WindowsDeviceError> {
            let path = path.as_ref().to_path_buf();
            let identifier = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| WindowsDeviceError::InvalidPath(path.display().to_string()))?;

            let info = disk_info(identifier)?;
            Self::open_with_device(path, info.device)
        }

        fn open_with_device(
            raw_path: PathBuf,
            mut device: PhysicalDevice,
        ) -> Result<Self, WindowsDeviceError> {
            device.platform_path = raw_path.display().to_string();
            let path_w = to_wchar(&raw_path.display().to_string());

            unsafe {
                let handle = windows_sys::Win32::Storage::FileSystem::CreateFileW(
                    path_w.as_ptr(),
                    GENERIC_READ,
                    windows_sys::Win32::Storage::FileSystem::FILE_SHARE_READ
                        | windows_sys::Win32::Storage::FileSystem::FILE_SHARE_WRITE,
                    std::ptr::null(),
                    windows_sys::Win32::Storage::FileSystem::OPEN_EXISTING,
                    windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL,
                    std::ptr::null_mut(),
                );

                if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
                    return Err(WindowsDeviceError::Io(std::io::Error::last_os_error()));
                }

                let file = File::from_raw_handle(handle as _);
                Ok(Self {
                    device,
                    file,
                    raw_path,
                })
            }
        }

        pub fn raw_path(&self) -> &Path {
            &self.raw_path
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

    impl BlockSource for WindowsRawDiskSource {
        fn device(&self) -> &PhysicalDevice {
            &self.device
        }

        fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<ReadResult, ReadError> {
            self.validate_range(offset, buffer.len())?;
            let bytes_read = self.file.seek_read(buffer, offset)?;
            Ok(ReadResult { offset, bytes_read })
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::*;

#[cfg(not(target_os = "windows"))]
mod stub {
    use data_recovery_core::{BlockSource, PhysicalDevice, ReadError, ReadResult};
    use std::path::{Path, PathBuf};

    #[derive(Debug)]
    pub enum WindowsDeviceError {
        UnsupportedPlatform,
    }

    impl std::fmt::Display for WindowsDeviceError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "此平台不支援 Windows 底層裝置存取")
        }
    }

    impl std::error::Error for WindowsDeviceError {}

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct WindowsDiskInfo {
        pub device: PhysicalDevice,
        pub device_node: String,
        pub content: Option<String>,
        pub mount_point: Option<String>,
        pub whole_disk: bool,
        pub writable_media: Option<bool>,
    }

    pub fn list_whole_disks() -> Result<Vec<WindowsDiskInfo>, WindowsDeviceError> {
        Ok(Vec::new())
    }

    pub fn disk_info(_identifier: &str) -> Result<WindowsDiskInfo, WindowsDeviceError> {
        Err(WindowsDeviceError::UnsupportedPlatform)
    }

    pub struct WindowsRawDiskSource {
        device: PhysicalDevice,
        raw_path: PathBuf,
    }

    impl WindowsRawDiskSource {
        pub fn open_identifier(_identifier: &str) -> Result<Self, WindowsDeviceError> {
            Err(WindowsDeviceError::UnsupportedPlatform)
        }

        pub fn open_path(_path: impl AsRef<Path>) -> Result<Self, WindowsDeviceError> {
            Err(WindowsDeviceError::UnsupportedPlatform)
        }

        pub fn raw_path(&self) -> &Path {
            &self.raw_path
        }
    }

    impl BlockSource for WindowsRawDiskSource {
        fn device(&self) -> &PhysicalDevice {
            &self.device
        }

        fn read_at(&self, _offset: u64, _buffer: &mut [u8]) -> Result<ReadResult, ReadError> {
            Err(ReadError::EmptyBuffer)
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub use stub::*;

#[cfg(test)]
mod tests {
    #[test]
    fn test_device_windows_stub() {
        #[cfg(not(target_os = "windows"))]
        {
            assert!(super::list_whole_disks().unwrap().is_empty());
            assert!(matches!(
                super::disk_info("C:").unwrap_err(),
                super::WindowsDeviceError::UnsupportedPlatform
            ));
        }
    }
}
