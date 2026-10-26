//! Facet values, parser, evaluator, and settings for hactorio.
//!
//! This crate has no dependency on egui and is testable headlessly.

pub mod approx;
mod belt;
pub mod facet_eval;
pub mod facet_format;
pub mod facet_machine;
pub mod facet_parser;
pub mod facet_token;
pub mod facets;
mod format;
pub mod new_eval;
mod settings;
mod syntax;
mod text;
mod value;

pub use belt::{BeltTier, belt_capacity};
pub use facet_eval::{
    EvalError, LineOutput, StatementOutput, Variables, eval_expr, eval_statement, evaluate,
    line_hash,
};
pub use facet_format::{format_capacity, format_config, format_facet, format_machine, format_rate};
pub use format::{format_amount, format_energy, format_power, format_value};
pub use settings::{Settings, TimeUnit};
pub use syntax::{BinaryOp, Highlight, Span, highlights};
pub use text::{from_plaintext, to_plaintext};
pub use value::{APPROX_ZERO, BaseUnit, Value, is_integer, is_nonzero, is_zero};
