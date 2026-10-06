# Privileged Boundary

> Status: production helper not implemented; raw-device operations fail closed.

## Current boundary

The canonical `0.2.0-alpha.1` source does not contain or launch the historical standalone root helper. The AppleScript elevation path, predictable temporary socket/token protocol, world-writable socket mode, privileged scan/extract server, automatic force-unmount behavior, and leftover scan/extract helper-mode CLI entry points were removed during source hardening.

CI runs `scripts/check-product-security.py` to reject those quarantined patterns if they reappear. This is a regression guard, not proof that the desktop product is ready for real devices.

Ordinary image-file parsing and synthetic-fixture tests run without elevated privileges. Desktop source opening rejects macOS and Windows raw-device path forms before attempting I/O; attempts to use the removed privileged path return an explicit unsupported error. No release artifact may claim raw-device support while this state remains.

## Requirements for a future macOS helper

A replacement design must be reviewed before implementation and must:

- be installed and signed as a dedicated system component using an Apple-supported mechanism;
- authenticate the client by code identity and validate every request independently;
- expose only narrow device enumeration and read-only open/read operations;
- resolve stable device identity again after open and reject the system disk by default;
- return a constrained handle or byte stream rather than accept arbitrary output paths;
- use owner-only IPC permissions and avoid secrets in command lines or predictable `/tmp` names;
- never scan filesystems, parse untrusted structures, write recovery output, force-unmount, or execute arbitrary commands as root;
- provide structured, privacy-safe audit events without logging recovered content or credentials;
- fail closed on identity mismatch, client authentication failure, unsupported topology, or missing entitlement.

Windows must receive an equivalent platform-specific threat model and least-privilege design rather than copying the macOS transport.

## Release gates

Raw-device support remains blocked until the replacement has:

1. a reviewed protocol and threat model;
2. code-signature and peer-authentication tests;
3. negative authorization and malformed-request tests;
4. proof that opened handles are read-only and stable-identity checked;
5. signed/notarized packaging and clean-machine install/uninstall evidence;
6. a verified source/output physical-store isolation preflight.

Exploit reproduction details for the removed prototype belong in a private security advisory, not in public documentation.
