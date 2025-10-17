//! Compiled component instances and hardware abstractions.
//!
//! This module provides the compiled representation of RIF components after parsing and compilation.
//! It handles the transformation from the parsed RIF structure ([`crate::rifgen`]) into a form
//! optimized for code generation, with abstractions that support multiple hardware description languages.
//!
//! # Overview
//!
//! The compilation process resolves all references, evaluates parameters, processes arrays,
//! handles partial fields, and creates the final register implementations. The compiled structures
//! in this module serve as the input to all code generators.
//!
//! # Main Components
//!
//! ## Component Instances
//!
//! The [`comp_inst`] module defines the core compiled structures:
//! - [`comp_inst::Comp`]: Top-level component (RIF, RifMux, or External)
//! - [`comp_inst::RifInst`]: Compiled RIF instance with pages and registers
//! - [`comp_inst::RifmuxInst`]: Compiled RIF multiplexer with address mappings
//! - [`comp_inst::RifPageInst`]: Compiled page with register instances
//!
//! ## Register Implementation
//!
//! The [`reg_impl`] module provides the compiled register and field structures:
//! - [`reg_impl::RegImpl`]: Complete register implementation after compilation
//! - [`reg_impl::FieldImpl`]: Field implementation with resolved properties
//! - [`reg_impl::HwRegs`]: Hardware register structures (SW and HW views)
//!
//! ## Hardware Information
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
//! # Compilation Process
//!
//! The compilation happens in [`comp_inst::Comp::compile`]:
//!
//! 1. **Parse** - RIF source is parsed into [`crate::rifgen`] structures
//! 2. **Resolve** - References, parameters, and includes are resolved
//! 3. **Compile** - Convert to [`comp_inst::Comp`] with all properties evaluated
//! 4. **Generate** - Code generators use compiled structures to produce output
//!

pub mod comp_inst;
pub mod reg_impl;
pub mod hw_info;