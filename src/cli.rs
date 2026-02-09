use std::error::Error;

use clap::Parser;

use crate::{
    cfg::{RifGenTarget, RtlLimit},
    generator::{casing::Casing, gen_py::PyVersion},
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
    #[arg(short = 'P', value_parser = parse_key_val::<String, isize>)]
    pub parameters: Vec<(String, isize)>,
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
    /// Base class for python target
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
    pub ral_class: Option<String>,
    /// Name of macro to create RAL register block
    #[arg(long)]
    pub ral_macro: Option<String>,
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
}

/// Parse a single key-value pair
fn parse_key_val<T, U>(s: &str) -> Result<(T, U), Box<dyn Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("Invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

