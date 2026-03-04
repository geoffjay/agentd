//! Command implementations for the agentd CLI.
//!
//! This module contains the implementation of all CLI subcommands. Each subcommand
//! is defined as an enum with variants for individual operations.
//!
//! # Command Structure
//!
//! Commands follow a hierarchical structure:
//! ```text
//! agentd
//!   ├─ notify (NotifyCommand)
//!   │   ├─ create
//!   │   ├─ list
//!   │   ├─ get
//!   │   ├─ delete
//!   │   └─ respond
//!   └─ ask (AskCommand)
//!       ├─ trigger
//!       └─ answer
//! ```
//!
//! # Adding New Commands
//!
//! To add a new command:
//! 1. Create a new module file (e.g., `foo.rs`)
//! 2. Define a `FooCommand` enum with clap `Subcommand` derive
//! 3. Implement an `execute()` method that takes `&ApiClient`
//! 4. Add the module and re-export in this file
//! 5. Add the command variant to `Commands` enum in `main.rs`

pub mod apply;
pub mod ask;
pub mod notify;
pub mod orchestrator;
pub mod wrap;

pub use ask::AskCommand;
pub use notify::NotifyCommand;
pub use orchestrator::OrchestratorCommand;
pub use wrap::WrapCommand;
