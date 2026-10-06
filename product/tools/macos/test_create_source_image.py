#!/usr/bin/env python3
"""Pure parser tests; these do not inspect or access real disks."""

from __future__ import annotations

import unittest

from create_source_image import (
    device_from_df_output,
    physical_stores_from_info,
    whole_disk_identifier,
)


class StorageIdentityParsingTests(unittest.TestCase):
    def test_normalizes_slice_to_whole_disk(self) -> None:
        self.assertEqual(whole_disk_identifier("/dev/disk12s3"), "disk12")
        self.assertIsNone(whole_disk_identifier("not-a-device"))

    def test_uses_parent_whole_disk_for_a_volume(self) -> None:
        info = {
            "DeviceIdentifier": "disk5s1",
            "ParentWholeDisk": "disk5",
            "WholeDisk": False,
        }
        self.assertEqual(physical_stores_from_info(info), {"disk5"})

    def test_preserves_all_apfs_physical_stores(self) -> None:
        info = {
            "DeviceIdentifier": "disk7",
            "APFSPhysicalStores": [
                {"DeviceIdentifier": "disk0s2"},
                {"APFSPhysicalStore": "/dev/disk2s4"},
            ],
        }
        self.assertEqual(physical_stores_from_info(info), {"disk0", "disk2"})

    def test_incomplete_identity_remains_unknown(self) -> None:
        self.assertEqual(physical_stores_from_info({"DeviceIdentifier": "disk3s1"}), set())

    def test_extracts_device_from_posix_df_output(self) -> None:
        output = (
            "Filesystem 512-blocks Used Available Capacity Mounted on\n"
            "/dev/disk3s5s1 1000 500 500 50% /System/Volumes/Data\n"
        )
        self.assertEqual(device_from_df_output(output), "/dev/disk3s5s1")

    def test_rejects_non_device_df_source(self) -> None:
        output = "Filesystem 512-blocks Used Available Capacity Mounted on\nmap auto_home 0 0 0 100% /System/Volumes/Data/home\n"
        with self.assertRaises(RuntimeError):
            device_from_df_output(output)


if __name__ == "__main__":
    unittest.main()
