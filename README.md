# agentd

A Rust workspace project with multiple daemon services and supporting libraries.

## Project Structure

This project uses Cargo workspaces with all crates located in the `crates/` directory:

### Daemon Services (with Tokio async runtime)

- **agentd-ask** - Ask daemon service
- **agentd-notify** - Notification daemon service
- **agentd-hook** - Hook daemon service
- **agentd-monitor** - Monitoring daemon service

All daemon services include:
- Tokio async runtime
- Graceful shutdown handling (Ctrl+C)
- Tracing/logging setup
- Basic daemon loop structure

### Libraries

- **agentd-wrap** - Wrap functionality library
- **agentd-ollama** - Ollama integration library

### CLI

- **agentd-cli** - Command-line interface with subcommands for managing daemons

## Building

Build all crates:
```bash
cargo build
```

Build a specific crate:
```bash
cargo build -p agentd-ask
```

## Running

Run individual daemon services:
```bash
cargo run -p agentd-ask
cargo run -p agentd-notify
cargo run -p agentd-hook
cargo run -p agentd-monitor
```

Run the CLI:
```bash
cargo run -p agentd-cli -- --help
```

## Development

Check all crates:
```bash
cargo check
```

Run tests:
```bash
cargo test
```

## License

MIT OR Apache-2.0
