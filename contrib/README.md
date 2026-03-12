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

Interactive installation script for end users.

**Features:**
- Guided installation process
- Colored terminal output
- Service auto-start option
- Comprehensive status checks

**Usage:**
```bash
./contrib/scripts/install.sh
```

**What it does:**
1. Checks prerequisites (macOS, Rust/Cargo)
2. Verifies project directory
3. Builds release binaries
4. Installs binaries to `/usr/local/bin/`
5. Installs plist files to `~/Library/LaunchAgents/`
6. Optionally starts services
7. Displays next steps

## Installation Methods

### cargo xtask (Recommended)
```bash
cargo xtask install-user
cargo xtask start-services
```

Type-safe Rust-based installer with better error handling.

### Interactive Script
```bash
./contrib/scripts/install.sh
```

Guided installation with prompts and colored output.

## See Also

- [INSTALL.md](../INSTALL.md) - Detailed installation guide
- [xtask/](../xtask/) - Rust-based installer implementation

## Contributing

To add new installation methods or modify existing ones:

1. Update the appropriate files in `contrib/`
2. Update all references in documentation
3. Test the installation process
4. Update this README

## License

MIT OR Apache-2.0 (same as parent project)
