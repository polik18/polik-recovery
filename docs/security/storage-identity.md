# Storage Identity Safety Contract

> Status: macOS and Windows regular-image source/output identity and opened-handle binding implemented; raw-device helpers remain unsupported.

The safe-writer now defines a `StorageIdentity` as a non-empty set of physical-store identifiers. Before a storage-verified write, `validate_storage_isolation` rejects either of these conditions:

- the source or output identity has no verified physical store;
- the source and output share one or more physical stores, even if their paths or logical volumes differ.

The check deliberately models a multi-store source or destination so APFS containers, RAID-like layouts and other composite storage cannot be reduced to one path string. Unit tests cover unknown source, unknown output, overlapping multi-store identities, disjoint identities and rejection before output creation.

The macOS image-creation script, desktop extraction backend, CLI and recovery-plan backend resolve the mounted device with `df -P`, then query `diskutil info -plist` for both the source and nearest existing output ancestor before creating output directories. They follow an APFS container reference, retain every physical store, and stop if resolution fails or the two sets overlap. Pure Rust and Python parser tests cover slices, ordinary volumes, multi-store APFS data, POSIX `df` output and incomplete identity.

Raw-device access remains unsupported. The generic file-backed source accepts only regular image files and rejects raw-device namespaces; the CLI no longer contains raw-info/raw-hash implementations. Quality gates use an explicit debug-only identity pair that first canonicalizes and confines source/output to `product/fixtures` and `product/target`; release builds reject that option.

Regular image sources are opened read-only with `O_NOFOLLOW` on Unix and validated as regular files from the opened handle rather than a preceding path lookup. The macOS resolver records the canonical source's device/inode and physical stores. After `FileBlockSource` opens the image, safe-writer obtains those facts again from that exact live file descriptor and issues a private `VerifiedSourceStorage` token only on an exact match. Every CLI, desktop and recovery-plan writer call requires this token, so verification cannot be replaced by reopening the path or by caller-asserted store strings.

On Windows, regular image files are opened with `FILE_FLAG_OPEN_REPARSE_POINT`; reparse sources and raw-device namespaces are rejected. The resolver obtains the volume GUID and `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS` disk-number set from opened source/output handles, records the source volume serial/file index, and verifies the parser's live file handle before issuing `VerifiedSourceStorage`. The Windows desktop now calls this same resolver rather than the former non-macOS fail-closed stub. A local two-disk test (`D:` source on disk 1, `%TEMP%` output on `C:` disk 2) completed a handle-bound write without opening either physical drive.

Before creating a partial file, the safe-writer queries the destination filesystem's user-available capacity and requires the exact recovered byte count plus a 5% margin (minimum 64 MiB) and a 64 KiB manifest reserve. On Unix/macOS it rejects every symbolic-link component in the output parent, canonicalizes that parent, opens it with `O_DIRECTORY | O_NOFOLLOW`, and compares the opened handle's device/inode with the expected directory. On macOS, identities carry private resolver provenance: the public writer rejects caller-asserted physical-store strings, uses `fstatfs` on the opened directory to obtain its real mount source, resolves that source through `diskutil`, and requires the resulting physical-store set to exactly match preflight. The only bypass is a debug-only fixture identity used by the separately path-confined synthetic quality command; it is absent from release builds.

Data and manifest `.partial` files are created relative to that same handle with `openat`, `O_EXCL` and `O_NOFOLLOW`; no-replace commits use `renameatx_np(..., RENAME_EXCL)` on macOS or handle-relative `linkat`/`unlinkat` on other Unix systems, followed by a sync of that same directory handle. Regression tests cover unproven identity rejection, real opened-handle store matching, mismatched store rejection, and replacement of the output path after its handle is open.

On Windows, every output path component is rejected if it is a reparse point. The destination directory is opened and checked again against its preflight physical extents. Partial files are created with `CREATE_NEW`, `FILE_FLAG_OPEN_REPARSE_POINT`, `DELETE` access and sharing that permits handle-based rename; before any recovered bytes are written, the opened file's resolved parent file identity must equal the verified directory handle. No-replace commit uses `SetFileInformationByHandle`, then rechecks the committed file's parent identity. Windows has no documented directory-`fsync` equivalent and `FlushFileBuffers` on a directory handle returns `ACCESS_DENIED`; both files are flushed before rename instead.

A successful call leaves no partial files. An interrupted or failed call can leave an explicitly named partial for diagnosis; callers must never present a `.partial` as a completed recovery.

The data/manifest pair is not an atomic two-file transaction. The implementation now recognizes and reconciles its only valid commit gap: final data exists, final manifest is absent, and the already-synced manifest `.partial` matches the final data's path, size and SHA-256. Mismatches and every other partial state fail closed without deleting evidence. A deterministic write-time disk-full injection proves that only a visibly named data `.partial` remains and no final output or manifest is exposed. The capacity check remains a preflight rather than a reservation, so concurrent disk use can still exhaust the destination.

Remaining release evidence includes Windows junction/mount-alias integration breadth, abrupt process-power-loss testing, formal MSVC validation, installer lifecycle, Authenticode and controlled removable-media tests. Four GNU release-profile desktop linked tests pass; none of the work above enables raw physical-drive reads.

No fallback may convert an identity-resolution error into a warning or user override.
