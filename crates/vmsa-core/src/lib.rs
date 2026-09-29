//! vmsa-core: platform-independent logic for the VM Setup Assistant.
//!
//! Responsibilities (see docs/ARCHITECTURE.md):
//! - `host`: inspect the host machine (OS, real CPU architecture, RAM, CPUs, disk, virtualization hints)
//! - `identity`: guest-visible VM identity configuration for compatibility testing (opt-in)
//! - `profile`: architecture-specific, version-aware VM configuration profiles and resource sizing
//! - `vbox`: typed VBoxManage command construction, execution and output parsing
//! - `download`: resumable, verified downloads from official sources
//! - `verify`: SHA-256 checksum verification and checksum-file parsing
//! - `iso`: ISO-9660 metadata inspection for Windows installation media
//! - `installer`: VirtualBox installer detection/handoff and result interpretation
//! - `state`: persistent setup state machine with recovery semantics
//! - `network`: guest network diagnosis and repair decisions
//! - `diagnostics`: redacted support reports
//! - `paths`: user-appropriate data directories

pub mod cmd;
pub mod diagnostics;
pub mod download;
pub mod error;
pub mod host;
pub mod identity;
pub mod installer;
pub mod iso;
pub mod network;
pub mod paths;
pub mod profile;
pub mod state;
pub mod vbox;
pub mod verify;

pub use error::{CoreError, Result};
