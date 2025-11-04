# Installation Guide for agentd

This guide covers installation of the agentd system on macOS.

## Overview

The agentd system consists of:

- **agent** - Command-line interface for interacting with services
- **Agent** - macOS GUI application (GPUI-based)
- **agentd-notify** - Notification service (REST API on port 3000)
- **agentd-ask** - Ask service for interactive questions (REST API on port 3001)
- **agentd-hook** - Hook service for shell integration
- **agentd-monitor** - Monitoring service

## Prerequisites

- macOS (tested on macOS 14+)
- Rust toolchain (1.75+)
- Git

## Installation Methods

Choose one of two installation methods:

1. **cargo xtask** - Type-safe Rust installer (recommended)
2. **Bash script** - Interactive installer

## Quick Start

### Option 1: cargo xtask (Recommended)

```bash
# Clone the repository
git clone https://github.com/yourusername/agentd.git
cd agentd

# Fix /usr/local permissions (one-time setup)
sudo chown -R $(whoami) /usr/local

# Install and start services
cargo xtask install-user
cargo xtask start-services
cargo xtask service-status
```

**Alternative:** Install to your home directory (no sudo needed):
```bash
# Install to ~/.local
PREFIX=$HOME/.local cargo xtask install-user

# Add to your PATH (add to ~/.zshrc or ~/.bashrc)
export PATH="$HOME/.local/bin:$PATH"

# Start services
cargo xtask start-services
```

### Option 2: Interactive Bash Script

```bash
./contrib/scripts/install.sh
```

The script will guide you through the installation process.

## Detailed Installation with cargo xtask

### Step 1: Fix Permissions (if using /usr/local)

```bash
# Give yourself write access to /usr/local (one-time)
sudo chown -R $(whoami) /usr/local
```

This allows installing to `/usr/local/bin` without sudo for each installation.

### Step 2: Install

```bash
cargo xtask install-user
```

This will:
1. Build all binaries in release mode
2. Install to `/usr/local/bin/` (or `$PREFIX/bin`)
3. Copy plist files to `~/Library/LaunchAgents/`
4. Create log directory at `/usr/local/var/log/`

### Step 3: Start Services

```bash
cargo xtask start-services
```

### Step 4: Verify

```bash
cargo xtask service-status
```

## What Gets Installed

**Binaries** (in `/usr/local/bin/` or `$PREFIX/bin`):
- `agent` - CLI
- `agentd-notify` - Notification service
- `agentd-ask` - Ask service
- `agentd-hook` - Hook service
- `agentd-monitor` - Monitor service
- `Agent` - GUI application

**Service Files** (in `~/Library/LaunchAgents/`):
- `com.geoffjay.agentd-notify.plist`
- `com.geoffjay.agentd-ask.plist`
- `com.geoffjay.agentd-hook.plist`
- `com.geoffjay.agentd-monitor.plist`

**Log Files** (in `/usr/local/var/log/` or `$PREFIX/var/log`):
- `agentd-notify.log` / `agentd-notify.err`
- `agentd-ask.log` / `agentd-ask.err`
- `agentd-hook.log` / `agentd-hook.err`
- `agentd-monitor.log` / `agentd-monitor.err`

## xtask Commands

```bash
# Installation
cargo xtask install-user    # Install for current user
cargo xtask install          # System-wide (requires sudo)

# Service Management
cargo xtask start-services   # Start all services
cargo xtask stop-services    # Stop all services
cargo xtask service-status   # Check service status

# Uninstallation
cargo xtask uninstall        # Remove everything
```

## CLI Usage

After installation, use the `agent` command:

```bash
# Create a notification
agent notify create --title "Test" --message "Hello" --priority high

# List notifications
agent notify list

# List only actionable notifications
agent notify list --actionable

# Get specific notification
agent notify get <UUID>

# Respond to a notification
agent notify respond <UUID> "My response"

# Delete a notification
agent notify delete <UUID>

# Trigger ask service checks
agent ask trigger

# Answer a question
agent ask answer <QUESTION_UUID> "yes"
```

## Service Management

### Using cargo xtask

```bash
# Check service status
cargo xtask service-status

# Stop services
cargo xtask stop-services

# Restart services
cargo xtask stop-services
cargo xtask start-services
```

### Manual Service Control

You can also use `launchctl` directly:

```bash
# Start a specific service
launchctl load ~/Library/LaunchAgents/com.geoffjay.agentd-notify.plist

# Stop a specific service
launchctl unload ~/Library/LaunchAgents/com.geoffjay.agentd-notify.plist

# Check if service is running
launchctl list | grep agentd

# View service status
launchctl list com.geoffjay.agentd-notify
```

## Configuration

### Service Ports

Default ports for services:
- **agentd-notify**: 3000
- **agentd-ask**: 3001

Ports can be configured via environment variables in the plist files.

### Custom Installation Location

Use the `PREFIX` environment variable to install to a custom location:

```bash
# Install to ~/.local
PREFIX=$HOME/.local cargo xtask install-user

# Install to /opt
PREFIX=/opt cargo xtask install-user
```

**Note:** When using a custom PREFIX, you may need to update plist files to use the correct binary paths.

### Log Files

Service logs are written to:
- Standard output: `/usr/local/var/log/agentd-<service>.log`
- Standard error: `/usr/local/var/log/agentd-<service>.err`

If using custom PREFIX:
- `$PREFIX/var/log/agentd-<service>.log`
- `$PREFIX/var/log/agentd-<service>.err`

View logs:
```bash
# View specific service log
tail -f /usr/local/var/log/agentd-notify.log

# Or with custom PREFIX
tail -f $HOME/.local/var/log/agentd-notify.log
```

## Troubleshooting

### Permission Denied Errors

If you get "Permission denied" during installation:

**Option 1: Fix /usr/local permissions (recommended)**
```bash
sudo chown -R $(whoami) /usr/local
cargo xtask install-user
```

**Option 2: Install to user directory**
```bash
PREFIX=$HOME/.local cargo xtask install-user
export PATH="$HOME/.local/bin:$PATH"
```

### Services Won't Start

1. Check if binaries are installed:
   ```bash
   ls -la /usr/local/bin/agentd-*
   # Or with custom PREFIX
   ls -la $PREFIX/bin/agentd-*
   ```

2. Check plist files are installed:
   ```bash
   ls -la ~/Library/LaunchAgents/com.geoffjay.agentd-*
   ```

3. Check for errors in logs:
   ```bash
   cat /usr/local/var/log/agentd-notify.err
   ```

4. Verify port availability:
   ```bash
   lsof -i :3000
   lsof -i :3001
   ```

### Service Keeps Restarting

Check logs for errors:
```bash
cat /usr/local/var/log/agentd-notify.err
```

Common issues:
- Database file permissions (for notify service)
- Port already in use
- Missing dependencies

### Cannot Connect to Service

1. Verify service is running:
   ```bash
   cargo xtask service-status
   ```

2. Check if port is listening:
   ```bash
   curl http://localhost:3000/health
   curl http://localhost:3001/health
   ```

3. Restart services:
   ```bash
   cargo xtask stop-services
   cargo xtask start-services
   ```

## Uninstallation

To completely remove agentd:

```bash
cargo xtask uninstall
```

This will:
1. Stop all services
2. Remove all binaries from `/usr/local/bin` (or `$PREFIX/bin`)
3. Remove plist files from LaunchAgents
4. Remove log files

Manual cleanup if needed:
```bash
# Remove database files
rm -rf ~/.local/share/agentd

# Remove configuration files (if any)
rm -rf ~/.config/agentd
```

## Development Installation

For development, you can run services directly without installing:

```bash
# Terminal 1: Run notify service
cargo run -p agentd-notify

# Terminal 2: Run ask service
cargo run -p agentd-ask

# Terminal 3: Use CLI
cargo run -p agentd-cli -- notify list
```

Or use the short alias:
```bash
# After building
cargo build --release

# Run directly
./target/release/agent notify list
```

## Next Steps

After installation:

1. Test the CLI:
   ```bash
   agent notify create --title "Test" --message "Installation successful!"
   agent notify list
   ```

2. Check service health:
   ```bash
   curl http://localhost:3000/health
   curl http://localhost:3001/health
   ```

3. View logs to ensure services are running:
   ```bash
   tail -f /usr/local/var/log/agentd-notify.log
   ```

4. Trigger an ask service check:
   ```bash
   agent ask trigger
   ```

## Getting Help

- Check logs: `tail -f /usr/local/var/log/agentd-*.log`
- Check status: `cargo xtask service-status`
- View all xtask commands: `cargo xtask`

## License

MIT OR Apache-2.0
