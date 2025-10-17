//! # YARIG: Yet Another Register Interface Generator
//!
//! YARIG is a code generation tool to describe register interface of an IP or ASIC/FPGA design.
//!
//! This allows to have one common source to describes registers for all the different views:
//! - **Hardware**: for the actual hardware implementation (SystemVerilog and VHDL)
//! - **Software**: for interacting with the register such a C header, UVM RAL, python classes, ...
//! - **Documentation**: for readable description (HTML/Latex/...)
//!
//! ## Quick Start
//!
//! YARIG uses its own file format named `.rif` to describe registers. The objectives when designing
//! the language were to make it easy to read and write, keep simple registers on a single line,
//! offer fine control on the generated hardware, and allow re-use and hierarchical description.
//!
//! ### Example
//!
//! ```text
//! rif: test_rif
//!   addrWidth: 8
//!   dataWidth: 16
//!   - Main:
//!     registers:
//!       - ctrl: "Basic control register"
//!         - en      = 0    0:0        "Block enable"
//!         - start   = 0    0:0  pulse "Start block"
//!         - version = 0x12 15:8 ro    "Block version"
//!           hw na
//!     instances: auto
//! ```
//!
//! ## Usage as a Library
//!
//! While YARIG is primarily used as a command-line tool, you can also use it as a library
//! in your Rust projects. The main modules are:
//!
//! - [`mod@cfg`]: Configuration management and generation orchestration
//! - [`parser`]: RIF file parsing
//! - [`rifgen`]: Parsed RIF data structures
//! - [`comp`]: Compiled component instances with hardware abstractions
//! - [`generator`]: Code generation for various output targets
//!
//! The typical flow is: **parse** → **compile** → **generate**
//!
//! See the [`cfg::YarigCfg`] documentation for examples of programmatic usage.
//!
//! ## Detailed Documentation

// RIF Syntax Documentation
#![doc = include_str!("../doc/syntax.md")]

// Configuration Documentation
#![doc = include_str!("../doc/config.md")]

// Package & Hardware Targets
#![doc = include_str!("../doc/package.md")]

// Other Targets
#![doc = include_str!("../doc/targets.md")]

pub mod error;
pub mod parser;
pub mod rifgen;
pub mod comp;
pub mod generator;
pub mod cfg;
pub mod cli;
