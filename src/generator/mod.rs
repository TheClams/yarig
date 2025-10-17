//! Code generation for various output targets.
//!
//! This module provides the infrastructure and implementations for generating code and documentation
//! from compiled RIF components ([`crate::comp`]).
//!
//! # Architecture
//!
//! ## Core Infrastructure
//!
//! - [`gen_common::GeneratorCore`]: Common structure providing text buffers, stashes, and utility
//!   methods shared by all generators
//! - [`gen_common::GeneratorBase`]: Base trait required by all generators, providing access to the
//!   core and file extension
//! - [`casing`]: Identifier casing utilities (snake_case, PascalCase, camelCase, etc.)
//!
//! ## Generator Traits
//!
//! Three specialized traits define common patterns for different generator categories:
//!
//! - [`trait_doc::GeneratorDoc`]: Documentation generators (HTML, LaTeX, AsciiDoc, MIF)
//! - [`trait_sw::GeneratorSw`]: Software interface generators (C, Python, JSON, RAL, SVD, IP-XACT)
//! - [`trait_hw::GeneratorHw`]: Hardware implementation generators (SystemVerilog, VHDL)
//!
//! Each trait provides default implementations for traversing the component structure and
//! generating appropriate output, with hooks for customization.
//!
//! # Generators
//!
//! ## Documentation Generators
//!
//! - [`gen_html::GeneratorHtml`]: HTML documentation with tables and styling
//! - [`gen_latex::GeneratorLatex`]: LaTeX documentation for technical documents
//! - [`gen_adoc::GeneratorAdoc`]: AsciiDoctor format for flexible documentation
//! - [`gen_mif::GeneratorMif`]: FrameMaker MIF format
//!
//! ## Software Generators
//!
//! - [`gen_c::GeneratorC`]: C header files with structures and macros
//! - [`gen_py::GeneratorPy`]: Python classes for register access
//! - [`gen_json::GeneratorJson`]: JSON representation of register maps
//! - [`gen_ral::GeneratorRal`]: UVM Register Abstraction Layer (RAL) for verification
//! - [`gen_svd::GeneratorSvd`]: CMSIS-SVD format for ARM tools
//! - [`gen_ipxact::GeneratorIpXact`]: IP-XACT standard format
//!
//! ## Hardware Generators
//!
//! - [`gen_sv::GeneratorSv`]: SystemVerilog RTL implementation
//! - [`gen_vhdl::GeneratorVhdl`]: VHDL RTL implementation
//!
//! Both hardware generators use [`crate::comp::hw_info`] abstractions to produce HDL-specific
//! output from the same compiled representation.

pub mod casing;
pub mod gen_common;
// Documentation generators
pub mod trait_doc;
pub mod gen_adoc;
pub mod gen_html;
pub mod gen_latex;
pub mod gen_mif;
// Software generators
pub mod trait_sw;
pub mod gen_c;
pub mod gen_ipxact;
pub mod gen_json;
pub mod gen_py;
pub mod gen_ral;
pub mod gen_svd;
// Hardware Generators
pub mod trait_hw;
pub mod gen_sv;
pub mod gen_vhdl;
