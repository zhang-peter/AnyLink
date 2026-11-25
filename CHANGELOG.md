# Changelog

All notable changes to AnyLink are recorded here. The application version is
read from `Cargo.toml` and shown in the window and Windows file properties.

## [0.2.0] - 2026-11-25

### Added

- AnyLink logo and Windows application icon
- Versioned Windows executable metadata, release archive, and in-app version badge
- Scrollable main page with a visible vertical scrollbar
- Searchable target-chip dropdown backed by the probe-rs target registry
- Intel HEX filter in the native firmware picker

### Changed

- The release GUI executable uses the Windows GUI subsystem, so double-clicking
  it does not open a console window
- Diagnostic commands moved to the separate `anylink-cli.exe`
- Removed the Rust / GPUI / probe-rs badge from the header
- Target selection now requires choosing an item from the dropdown; typing only
  filters the available targets

### Fixed

- A HEX file without any data in the target NVM region could previously be
  reported as successfully downloaded
- Long pages could be clipped instead of scrolling when the window was small

## [0.1.0] - 2026-11-25

### Added

- Initial DAP-Link flashing application
- probe-rs target-chip lookup
- Intel HEX download, erase, program, and optional verification
- Background flashing with progress and logs
- Configuration persistence for probe, target, firmware, and verification setting
