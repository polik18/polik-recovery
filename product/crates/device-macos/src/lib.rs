use data_recovery_core::{BlockSource, BusType, PhysicalDevice, ReadError, ReadResult};
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
#[cfg(windows)]
use std::os::windows::fs::FileExt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub enum MacOsDeviceError {
    CommandFailed {
        command: String,
        status: Option<i32>,
        stderr: String,
    },
    MissingField {
        field: &'static str,
        identifier: String,
    },
    ParseInt {
        field: &'static str,
        value: String,
    },
    Io(std::io::Error),
    Read(ReadError),
}

impl std::fmt::Display for MacOsDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CommandFailed {
                command,
                status,
                stderr,
            } => write!(
                f,
                "{command} failed with status {:?}: {}",
                status,
                stderr.trim()
            ),
            Self::MissingField { field, identifier } => {
                write!(f, "diskutil plist for {identifier} is missing {field}")
            }
            Self::ParseInt { field, value } => {
                write!(f, "failed to parse {field} as integer: {value}")
            }
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Read(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for MacOsDeviceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Read(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for MacOsDeviceError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<ReadError> for MacOsDeviceError {
    fn from(value: ReadError) -> Self {
        Self::Read(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MacOsDiskInfo {
    pub device: PhysicalDeviceSerializable,
    pub device_node: String,
    pub content: Option<String>,
    pub mount_point: Option<String>,
    pub whole_disk: bool,
    pub writable_media: Option<bool>,
}

// Mirror PhysicalDevice for serialization compatibility with helper
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PhysicalDeviceSerializable {
    pub id: String,
    pub display_name: String,
    pub platform_path: String,
    pub size_bytes: u64,
    pub logical_sector_size: u32,
    pub physical_sector_size: u32,
    pub is_removable: bool,
    pub is_system_disk: bool,
    pub bus_type: String,
    pub trim_supported: Option<bool>,
}

fn map_to_serializable(dev: &PhysicalDevice) -> PhysicalDeviceSerializable {
    PhysicalDeviceSerializable {
        id: dev.id.clone(),
        display_name: dev.display_name.clone(),
        platform_path: dev.platform_path.clone(),
        size_bytes: dev.size_bytes,
        logical_sector_size: dev.logical_sector_size,
        physical_sector_size: dev.physical_sector_size,
        is_removable: dev.is_removable,
        is_system_disk: dev.is_system_disk,
        bus_type: format!("{:?}", dev.bus_type),
        trim_supported: dev.trim_supported,
    }
}

fn map_from_serializable(ser: &PhysicalDeviceSerializable) -> PhysicalDevice {
    let bus_type = match ser.bus_type.as_str() {
        "Ata" => BusType::Ata,
        "Sata" => BusType::Sata,
        "Nvme" => BusType::Nvme,
        "Usb" => BusType::Usb,
        "Thunderbolt" => BusType::Thunderbolt,
        "Virtual" => BusType::Virtual,
        _ => BusType::Unknown,
    };
    PhysicalDevice {
        id: ser.id.clone(),
        display_name: ser.display_name.clone(),
        platform_path: ser.platform_path.clone(),
        size_bytes: ser.size_bytes,
        logical_sector_size: ser.logical_sector_size,
        physical_sector_size: ser.physical_sector_size,
        is_removable: ser.is_removable,
        is_system_disk: ser.is_system_disk,
        bus_type,
        trim_supported: ser.trim_supported,
    }
}

pub fn list_whole_disks() -> Result<Vec<MacOsDiskInfo>, MacOsDeviceError> {
    let plist = run_diskutil(&["list", "-plist"])?;
    let identifiers = parse_string_array_after_key(&plist, "WholeDisks");
    let mut devices = Vec::with_capacity(identifiers.len());

    for identifier in identifiers {
        devices.push(disk_info(&identifier)?);
    }

    Ok(devices)
}

pub fn disk_info(identifier: &str) -> Result<MacOsDiskInfo, MacOsDeviceError> {
    let plist = run_diskutil(&["info", "-plist", identifier])?;
    parse_disk_info_plist(identifier, &plist)
}

pub struct MacOsRawDiskSource {
    device: PhysicalDevice,
    file: File,
    raw_path: PathBuf,
}

impl MacOsRawDiskSource {
    pub fn open_identifier(identifier: &str) -> Result<Self, MacOsDeviceError> {
        let info = disk_info(identifier)?;
        let device = map_from_serializable(&info.device);
        let raw_path = raw_path_for_identifier(&device.id);
        Self::open_with_device(raw_path, device)
    }

    pub fn open_path(path: impl AsRef<Path>) -> Result<Self, MacOsDeviceError> {
        let path = path.as_ref().to_path_buf();
        let identifier = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_prefix('r').or(Some(name)))
            .ok_or_else(|| MacOsDeviceError::MissingField {
                field: "device identifier",
                identifier: path.display().to_string(),
            })?;

        let info = disk_info(identifier)?;
        let device = map_from_serializable(&info.device);
        Self::open_with_device(path, device)
    }

    fn open_with_device(
        raw_path: PathBuf,
        mut device: PhysicalDevice,
    ) -> Result<Self, MacOsDeviceError> {
        device.platform_path = raw_path.display().to_string();
        let file = File::open(&raw_path)?;
        Ok(Self {
            device,
            file,
            raw_path,
        })
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

impl BlockSource for MacOsRawDiskSource {
    fn device(&self) -> &PhysicalDevice {
        &self.device
    }

    fn read_at(&self, offset: u64, buffer: &mut [u8]) -> Result<ReadResult, ReadError> {
        self.validate_range(offset, buffer.len())?;

        #[cfg(unix)]
        let bytes_read = self.file.read_at(buffer, offset)?;
        #[cfg(windows)]
        let bytes_read = self.file.seek_read(buffer, offset)?;
        Ok(ReadResult { offset, bytes_read })
    }
}

fn run_diskutil(args: &[&str]) -> Result<String, MacOsDeviceError> {
    let output = Command::new("diskutil").args(args).output()?;
    if !output.status.success() {
        return Err(MacOsDeviceError::CommandFailed {
            command: format!("diskutil {}", args.join(" ")),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_disk_info_plist(
    requested_identifier: &str,
    plist: &str,
) -> Result<MacOsDiskInfo, MacOsDeviceError> {
    let identifier =
        string_value(plist, "DeviceIdentifier").ok_or_else(|| MacOsDeviceError::MissingField {
            field: "DeviceIdentifier",
            identifier: requested_identifier.to_string(),
        })?;
    let size = integer_value(plist, "TotalSize")
        .or_else(|| integer_value(plist, "Size"))
        .ok_or_else(|| MacOsDeviceError::MissingField {
            field: "TotalSize",
            identifier: identifier.clone(),
        })?;
    let block_size = integer_value(plist, "DeviceBlockSize").unwrap_or(512);
    let device_node =
        string_value(plist, "DeviceNode").unwrap_or_else(|| format!("/dev/{identifier}"));
    let display_name = string_value(plist, "MediaName")
        .or_else(|| string_value(plist, "VolumeName"))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| identifier.clone());
    let bus_type = string_value(plist, "BusProtocol")
        .as_deref()
        .map(map_bus_type)
        .unwrap_or(BusType::Unknown);

    let dev = PhysicalDevice {
        id: identifier,
        display_name,
        platform_path: device_node.clone(),
        size_bytes: size,
        logical_sector_size: block_size as u32,
        physical_sector_size: block_size as u32,
        is_removable: bool_value(plist, "Removable")
            || bool_value(plist, "RemovableMedia")
            || bool_value(plist, "RemovableMediaOrExternalDevice"),
        is_system_disk: bool_value(plist, "Bootable")
            || bool_value(plist, "SystemImage")
            || string_value(plist, "MountPoint").as_deref() == Some("/"),
        bus_type,
        trim_supported: bool_value_opt(plist, "SolidState"),
    };

    Ok(MacOsDiskInfo {
        device: map_to_serializable(&dev),
        device_node,
        content: string_value(plist, "Content").filter(|value| !value.is_empty()),
        mount_point: string_value(plist, "MountPoint").filter(|value| !value.is_empty()),
        whole_disk: bool_value(plist, "WholeDisk"),
        writable_media: bool_value_opt(plist, "WritableMedia"),
    })
}

fn raw_path_for_identifier(identifier: &str) -> PathBuf {
    PathBuf::from(format!("/dev/r{identifier}"))
}

fn map_bus_type(value: &str) -> BusType {
    let lower = value.to_ascii_lowercase();
    if lower.contains("nvme") || lower.contains("apple fabric") {
        BusType::Nvme
    } else if lower.contains("usb") {
        BusType::Usb
    } else if lower.contains("thunderbolt") {
        BusType::Thunderbolt
    } else if lower.contains("sata") {
        BusType::Sata
    } else if lower.contains("ata") {
        BusType::Ata
    } else {
        BusType::Unknown
    }
}

fn parse_string_array_after_key(plist: &str, key: &str) -> Vec<String> {
    let Some(key_pos) = plist.find(&format!("<key>{key}</key>")) else {
        return Vec::new();
    };
    let rest = &plist[key_pos..];
    let Some(array_start) = rest.find("<array>") else {
        return Vec::new();
    };
    let array_rest = &rest[array_start + "<array>".len()..];
    let Some(array_end) = array_rest.find("</array>") else {
        return Vec::new();
    };

    collect_strings(&array_rest[..array_end])
}

fn string_value(plist: &str, key: &str) -> Option<String> {
    value_after_key(plist, key, "string").map(xml_unescape)
}

fn integer_value(plist: &str, key: &str) -> Option<u64> {
    value_after_key(plist, key, "integer").and_then(|value| value.parse::<u64>().ok())
}

fn bool_value(plist: &str, key: &str) -> bool {
    bool_value_opt(plist, key).unwrap_or(false)
}

fn bool_value_opt(plist: &str, key: &str) -> Option<bool> {
    let key_marker = format!("<key>{key}</key>");
    let key_pos = plist.find(&key_marker)?;
    let rest = &plist[key_pos + key_marker.len()..];
    let trimmed = rest.trim_start();
    if trimmed.starts_with("<true/>") {
        Some(true)
    } else if trimmed.starts_with("<false/>") {
        Some(false)
    } else {
        None
    }
}

fn value_after_key<'a>(plist: &'a str, key: &str, tag: &str) -> Option<&'a str> {
    let key_marker = format!("<key>{key}</key>");
    let key_pos = plist.find(&key_marker)?;
    let rest = &plist[key_pos + key_marker.len()..];
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let open_pos = rest.find(&open)?;
    let value_start = open_pos + open.len();
    let close_pos = rest[value_start..].find(&close)?;
    Some(rest[value_start..value_start + close_pos].trim())
}

fn collect_strings(xml: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut rest = xml;

    while let Some(start) = rest.find("<string>") {
        rest = &rest[start + "<string>".len()..];
        let Some(end) = rest.find("</string>") else {
            break;
        };
        values.push(xml_unescape(rest[..end].trim()));
        rest = &rest[end + "</string>".len()..];
    }

    values
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST_PLIST: &str = r#"
<plist version="1.0">
<dict>
  <key>WholeDisks</key>
  <array>
    <string>disk0</string>
    <string>disk4</string>
  </array>
</dict>
</plist>
"#;

    const INFO_PLIST: &str = r#"
<plist version="1.0">
<dict>
  <key>BusProtocol</key>
  <string>USB</string>
  <key>Content</key>
  <string>GUID_partition_scheme</string>
  <key>DeviceBlockSize</key>
  <integer>4096</integer>
  <key>DeviceIdentifier</key>
  <string>disk4</string>
  <key>DeviceNode</key>
  <string>/dev/disk4</string>
  <key>MediaName</key>
  <string>Test Disk</string>
  <key>MountPoint</key>
  <string></string>
  <key>Removable</key>
  <true/>
  <key>Size</key>
  <integer>1048576</integer>
  <key>TotalSize</key>
  <integer>1048576</integer>
  <key>WholeDisk</key>
  <true/>
  <key>WritableMedia</key>
  <true/>
</dict>
</plist>
"#;

    #[test]
    fn parses_whole_disk_identifiers() {
        assert_eq!(
            parse_string_array_after_key(LIST_PLIST, "WholeDisks"),
            vec!["disk0", "disk4"]
        );
    }

    #[test]
    fn parses_disk_info() {
        let info = parse_disk_info_plist("disk4", INFO_PLIST).unwrap();

        assert_eq!(info.device.id, "disk4");
        assert_eq!(info.device.display_name, "Test Disk");
        assert_eq!(info.device.platform_path, "/dev/disk4");
        assert_eq!(info.device.size_bytes, 1_048_576);
        assert_eq!(info.device.logical_sector_size, 4096);
        assert_eq!(info.device.bus_type, "Usb");
        assert!(info.device.is_removable);
        assert!(info.whole_disk);
    }
}
