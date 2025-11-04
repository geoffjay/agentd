# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- CLI (`agent`) with comprehensive notification and ask service commands
- Ask service with tmux integration and REST API
- Notification service with SQLite storage and REST API
- Comprehensive test suite (148+ tests)
- Two installation methods: `cargo xtask` and bash script
- macOS LaunchAgent plist files for service management
- Interactive installation script (`contrib/scripts/install.sh`)
- Rust-based installer (`cargo xtask`)
- `contrib/` directory for installation scripts and plist files
- Comprehensive Rust documentation (1,000+ lines)

### Changed
- Renamed CLI binary from `agentd` to `agent`
- Renamed GUI binary from `ui` to `Agent`
- Moved `plists/` to `contrib/plists/`
- Moved `scripts/` to `contrib/scripts/`

### Fixed
- Updated all documentation references to new directory structure
- Updated xtask paths for contrib directory
- Resolved all clippy warnings and formatting issues

### Removed
- Makefile (simplified to cargo xtask only)

## [0.1.0] - 2024-11-04

### Added
- Initial project structure with Cargo workspace
- Basic daemon services (notify, ask, hook, monitor)
- CLI skeleton
- Project documentation

[Unreleased]: https://github.com/yourusername/agentd/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/yourusername/agentd/releases/tag/v0.1.0
