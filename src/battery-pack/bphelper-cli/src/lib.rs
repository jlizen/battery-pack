//! CLI for battery-pack: create and manage battery packs.

mod commands;
pub(crate) mod manifest;
pub(crate) mod registry;
pub(crate) mod template_engine;
mod tui;

// The only true public API
pub use commands::main;
pub use commands::validate::validate_templates;
