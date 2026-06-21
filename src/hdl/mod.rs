//! Structure and parser related to HDL (Hardwrae description language) generator
//!
//! # Hardware Information
//!
//! The [`hw_info`] module provides HDL-agnostic abstractions:
//! - [`hw_info::SignalKind`]: Signal types (unsigned, signed, custom, address, data)
//! - [`hw_info::SignalDim`]: Signal dimensions (fixed or generic)
//! - [`hw_info::SignalDef`]: Signal definition with type and dimensions
//! - [`hw_info::PortInfo`]: Port information with direction and signal properties
//! - [`hw_info::PortList`]: Collection of ports for a module/entity
//!
//! These abstractions allow generators to produce output for different HDLs (SystemVerilog,
//! VHDL) from the same compiled representation.
//!
//! # Logical expression
//!
//! The [`logic_expr`] module provides absatraction for logical expression use in HDL generator
//! and also in RIF definition for some signal propeties such as enable/clear signal, ...

pub mod hw_info;
pub mod logic_expr;
pub mod parser_sv;

// Re-export at top for easier use
pub use {
	hw_info::*,
	logic_expr::*,
};