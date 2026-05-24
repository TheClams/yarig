use std::error::Error;

use clap::Parser;

use crate::{
    cfg::{RifGenTarget, RtlLimit},
    generator::{casing::Casing, gen_common::Skippable, gen_py::PyVersion},
    rifgen::{Interface, SuffixInfo}
};


#[derive(Parser)]
#[command(version, rename_all="snake_case")]
/// Command-line arguments for Yarig
pub struct RifGenArgs{
    /// path to the RIF file to parse
    #[arg(short, long)]
    pub rif: Option<String>,
    /// path to a config file
    #[arg(short, long)]
    pub cfg: Option<String>,
    /// path to the RIF file to parse
    #[arg(short, long)]
    pub include: Vec<String>,
    /// List of targets
    #[arg(short, long, num_args = 1..)]
    pub targets: Vec<RifGenTarget>,
    /// Check syntax only, generator are not called
    #[arg(long, action)]
    pub check: bool,
    /// Use legacy order for interrupts (mask before enable)
    #[arg(long, action)]
    pub auto_legacy: bool,
    /// List of included component to generate. Use "*" to select all.
    #[arg(long, num_args = 0..)]
    pub gen_inc: Vec<String>,
    /// Output path for C header
    #[arg(long)]
    pub output_c: Option<String>,
    /// Output path for Python classes
    #[arg(long)]
    pub output_py: Option<String>,
    /// Output path for documentation output (HTML, latex, ...)
    #[arg(long)]
    pub output_doc: Option<String>,
    /// Output path for JSON output
    #[arg(long)]
    pub output_json: Option<String>,
    /// Output path for hardware output (SV, VHDL)
    #[arg(long)]
    pub output_rtl: Option<String>,
    /// Output path for simulation output (RAL)
    #[arg(long)]
    pub output_sim: Option<String>,
    /// Sub-directory name for generated file which are not the top level
    #[arg(long)]
    pub subdir: Option<String>,
    /// Public documentation (hide all private registers/fields)
    #[arg(long, action)]
    pub public: bool,
    /// Set parameters value
    #[arg(short = 'P', long, value_parser = parse_key_val)]
    pub parameters: Vec<(String, isize)>,
    /// Set parameters value
    #[arg(short = 'G', long, value_parser = parse_key_val)]
    pub doc_generics: Vec<(String, isize)>,
    /// Set suffix value
    #[arg(short = 'S', long)]
    pub suffix: Option<SuffixInfo>,
    /// List of target which should split their output. Supported targets are: html, mif, adoc
    #[arg(long, num_args = 0..)]
    pub split: Vec<RifGenTarget>,
    /// Rename field using reserved keyword
    #[arg(long, action)]
    pub keyword_rename: bool,
    /// Use suffix only  for RTL outputs
    #[arg(long, action)]
    pub suffix_rtl_only: bool,
    /// Specify an HDL interface
    #[arg(long)]
    pub interface: Option<Interface>,
    /// Specify casing used in all targets
    #[arg(long)]
    pub casing: Option<Casing>,
    /// C macro name defining the base address of the top level
    #[arg(long)]
    pub c_base_addr_name: Option<String>,
    /// Prefix C pointer for the top RifMux
    #[arg(long)]
    pub c_prefix_top_ptr: Option<String>,
    /// Base address offset in documentation
    #[arg(long, value_parser=parse_usize)]
    pub doc_base_offset: Option<usize>,
    /// Base class for python target (default Regmap)
    #[arg(long)]
    pub py_class: Option<String>,
    /// Python version (default 3.11)
    #[arg(long)]
    pub py_version: Option<PyVersion>,
    /// Python create Init File
    #[arg(long)]
    pub py_init_file: Option<bool>,
    /// Base class for RAL target
    #[arg(long)]
    pub ral_base_class: Option<String>,
    /// Name of macro to create RAL register block
    #[arg(long)]
    pub ral_block_macro: Option<String>,
    /// RAL use the UVM base class instead of extending another RAL (when derived)
    #[arg(long)]
    pub ral_force_base: Option<bool>,
    /// Add constant in RTL package for registers (address/reset)
    #[arg(long, action)]
    pub rtl_const_reg: bool,
    /// Add constant in RTL package for fields (mask/position/reset)
    #[arg(long, action)]
    pub rtl_const_field: bool,
    /// Controls how field limits are used
    #[arg(long, action)]
    pub rtl_limit: Option<RtlLimit>,
    /// Force generation of limits on all enums
    #[arg(long, action)]
    pub rtl_force_limit: Option<RtlLimit>,
    /// Add pipe level in Rifmux when generating invalid address signal due to no RIF being selected
    #[arg(long)]
    pub rifmux_pipe_invalid: Option<bool>,
    /// List of element to skip in generators (ADOC only at the moment): RifTitle, RifmuxTitle
    #[arg(long, num_args = 1..)]
    pub skip: Vec<Skippable>,
}

/// Parse a single key-value pair
pub fn parse_key_val(s: &str) -> Result<(String, isize), Box<dyn Error + Send + Sync + 'static>> {
    let pos = s
        .find('=')
        .ok_or_else(|| format!("Invalid KEY=value: no `=` found in `{s}`"))?;
    let key = s[..pos].to_string();
    let value_str = &s[pos + 1..];

    let value = match value_str.as_bytes() {
        [b'0', b'x' | b'X', ..] => isize::from_str_radix(&value_str[2..], 16)?,
        [b'0', b'b' | b'B', ..] => isize::from_str_radix(&value_str[2..], 2)?,
        [b'F'|b'f', b'a', b'l', b's', b'e' ] => 0,
        [b'T'|b't', b'r', b'u', b'e' ] => 1,
        _ => value_str.parse()?,
    };

    Ok((key, value))

}

/// Parse a single key-value pair
pub fn parse_usize(s: &str) -> Result<usize, Box<dyn Error + Send + Sync + 'static>> {
    let value = match s.as_bytes() {
        [b'0', b'x' | b'X', ..] => usize::from_str_radix(&s[2..], 16)?,
        [b'0', b'b' | b'B', ..] => usize::from_str_radix(&s[2..], 2)?,
        _ => s.parse()?,
    };
    Ok(value)
}

