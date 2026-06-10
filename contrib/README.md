# contrib/

This directory contains contributed files and utilities for agentd installation and configuration.

## Directory Structure

```
contrib/
├── plists/          # macOS LaunchAgent/LaunchDaemon plist files
│   ├── com.geoffjay.agentd-notify.plist
│   ├── com.geoffjay.agentd-ask.plist
│   ├── com.geoffjay.agentd-hook.plist
│   └── com.geoffjay.agentd-monitor.plist
└── scripts/         # Installation and utility scripts
    └── install.sh   # Interactive installation script
```

## Contents

### plists/

macOS service configuration files for running agentd services via `launchd`.

**Features:**
- Auto-start on boot (`RunAtLoad`)
- Auto-restart on crash (`KeepAlive`)
- Logging to `/usr/local/var/log/`
- Environment variable configuration

**Installation:**
```bash
# User installation (recommended)
cp contrib/plists/*.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.geoffjay.agentd-notify.plist

# Or use cargo xtask
cargo xtask install-user
```

**Service Configuration:**

Each service is configured with a production port (set via `AGENTD_PORT` env var in the plist):
- **ask** - Port 7001
- **hook** - Port 7002
- **monitor** - Port 7003
- **notify** - Port 7004
- **wrap** - Port 7005
- **orchestrator** - Port 7006

### scripts/

Installation and utility scripts for agentd.

#### install.sh

POSIX `sh` installer for prebuilt release binaries. Uploaded to every GitHub
Release so it can be piped straight from `releases/latest/download/install.sh`.

**Usage:**
```bash
curl -fsSL https://github.com/geoffjay/agentd/releases/latest/download/install.sh | sh
```

**What it does:**
1. Detects the platform (Linux x86_64/aarch64 musl, macOS Intel/Apple Silicon)
2. Resolves the latest release tag (or `AGENTD_VERSION` to pin a version)
3. Downloads the release tarball and `SHA256SUMS`, and verifies the checksum
4. Extracts to a temporary directory
5. Runs `agent install` for the platform-specific setup (binaries, launchd/systemd services, config, web UI, database migrations)

No Rust toolchain or source tree is required; all installation logic lives in
the `agentd-install` crate, this script only fetches and verifies artifacts.
`PREFIX` overrides the install prefix.

## Installation Methods

### Release installer (Recommended)
```bash
curl -fsSL https://github.com/geoffjay/agentd/releases/latest/download/install.sh | sh
```

Prebuilt binaries from GitHub Releases, verified via `SHA256SUMS`.

### cargo xtask (from source)
```bash
cargo xtask install-user
cargo xtask start-services
```

Builds binaries and the UI locally, then performs the same platform setup.

## See Also

- [docs/public/install.md](../docs/public/install.md) - Detailed installation guide
- [crates/install/](../crates/install/) - Rust installation library
- [crates/xtask/](../crates/xtask/) - Dev-only build + install front-end

## Contributing

To add new installation methods or modify existing ones:

1. Update the appropriate files in `contrib/`
2. Update all references in documentation
3. Test the installation process
4. Update this README

## License

MIT OR Apache-2.0 (same as parent project)
