//! The expression language, settings, and text helpers for hactorio.
//!
//! This crate has no dependency on egui and is testable headlessly.

pub mod approx;
pub mod belt;
pub mod lang;
mod settings;
mod syntax;
mod text;

pub use settings::{IdlePower, Settings, TimeUnit};
pub use syntax::{BinaryOp, Highlight, Span, highlights};
pub use text::{from_plaintext, to_plaintext};
