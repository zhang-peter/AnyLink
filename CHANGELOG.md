# Changelog

All notable changes to AnyLink are recorded here. The application version is
read from `Cargo.toml` and shown in the window and Windows file properties.

## [0.4.1] - 2026-11-25

### Changed

- "复位运行" is now enabled by default for new and pre-existing configurations
  that do not yet contain the setting
- The last selected firmware file path is restored on startup and logged
- The last firmware directory is persisted separately so the file picker can
  reopen there even if the previous file has moved

## [0.4.0] - 2026-11-25

### Added

- "复位运行" / reset-and-run after a successful download, configurable and persisted
- "仅整片擦除" action that mass-erases all nonvolatile memory without downloading
- Confirmation step before the destructive erase-only action
- `anylink-cli --erase-chip [target]` for hardware erase diagnostics
- GUI progress/result feedback for erase-only operations

### Changed

- A successful download now explicitly reports whether the target was reset and
  left running or remains paused
- The erase-only operation and the download operation share busy state and are
  mutually exclusive

### Verified

- Real erase-only operation on an STM32G491RE; start and end of the 512 KiB flash
  both read back as erased (`FF`)
- Real download with reset-after-flash on the same target; core status read back
  independently as `Running`
- Confirmation UI for the destructive erase-only action

## [0.3.1] - 2026-11-25

### Fixed

- HEX files using double-CR line endings (`\r\r\n`), blank lines between records,
  leading/trailing whitespace, or a UTF-8 BOM no longer fail with
  `Failed to read or decode the IHEX file`
- HEX parsing now happens before the debug probe is opened, so file errors are
  reported without requiring a target connection

### Added

- `anylink-cli --validate-hex <firmware.hex>` for hardware-independent HEX validation
- Parse/download summary includes HEX record count, data bytes, and address range
- Regression test for tolerant HEX line normalization

## [0.3.0] - 2026-11-25

### Added

- Optional full-chip erase (`do_chip_erase`) with a destructive-operation warning
- ST-Link / ST-Link V2/V2-1/V3 support through the probe-rs ST-Link driver
- Probe cards now distinguish DAP-Link / CMSIS-DAP and ST-Link families

### Changed

- The probe scanner now lists all supported CMSIS-DAP and ST-Link probes
- Download results and logs include the selected erase mode
- Full-chip erase is persisted with the other application settings

### Verified

- Real full-chip erase, programming, and verification on an STM32H743ZI test board
- Flash tail at `0x081FFF00` reads back as erased (`FF`) after full-chip erase

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
