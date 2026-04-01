//! Configuration management for YARIG code generation.
//!
//! This module provides the configuration structures used to control the parsing and generation
//! of register interface files (RIF). The configuration can be loaded from a TOML file and/or
//! built from command-line arguments.
//!
//! # Overview
//!
//! The main entry point is the [`YarigCfg`] structure, which contains:
//! - Input file specification and search paths
//! - List of target formats to generate (SystemVerilog, C, Python, HTML, etc.)
//! - Global settings (visibility, casing, parameters, etc.)
//! - Target-specific configuration options
//!
//! # Usage
//!
//! ## Loading from TOML file and generate all targets
//!
//! ```no_run
//! use yarig::cfg::YarigCfg;
//! let cfg = YarigCfg::from_file("config.toml")
//!     .expect("Failed to load configuration");
//! cfg.gen_all()
//!     .expect("Generation failed");
//! ```
//!
//! # Configuration File Format
//!
//! The configuration file uses TOML format. Here's a minimal example:
//!
//! ```toml
//! filename = "my_chip.rif"
//! targets = ["sv", "c", "html"]
//!
//! [outputs]
//! rtl = "./generated/rtl"
//! c = "./generated/include"
//! html = "./docs"
//! ```
//!
//! For detailed configuration options, see the
//! [configuration documentation](https://github.com/TheClams/rifgen/blob/dev/doc/config.md).
//!
//! # Supported Targets
//!
//! The [`RifGenTarget`] enum defines all available generation targets:
//!
//! - **Hardware**: SystemVerilog (`sv`), VHDL (`vhdl`)
//! - **Software**: C headers (`c`), Python classes (`py`), UVM RAL (`ral`)
//! - **Documentation**: HTML (`html`), LaTeX (`latex`), AsciiDoc (`adoc`), Framemaker (`mif`)
//! - **Data formats**: JSON (`json`), SVD (`svd`), IP-XACT (`ipxact`)
//!
//! Each target has its own configuration structure (e.g., [`CfgHtml`], [`CfgC`], [`CfgRtl`])
//! that allows fine-tuning the output for that specific target.

use serde_derive::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};
use toml;
use crate::{
    cli::RifGenArgs, comp::comp_inst::{Comp, RifFieldInst}, generator::{
        casing::Casing, gen_adoc::GeneratorAdoc, gen_c::GeneratorC, gen_common::{GeneratorBaseSetting, Skippable}, gen_html::GeneratorHtml, gen_ipxact::GeneratorIpXact, gen_json::GeneratorJson, gen_latex::GeneratorLatex, gen_mif::GeneratorMif, gen_py::{GeneratorPy, PyVersion}, gen_ral::GeneratorRal, gen_sv::GeneratorSv, gen_svd::GeneratorSvd, gen_vhdl::GeneratorVhdl, trait_doc::GeneratorDoc, trait_hw::GeneratorHw, trait_sw::GeneratorSw
    }, parser::{ParserCfg, RifGenSrc, RsvdKeywordSel, parser_expr::ParamValues}, rifgen::{Interface, SuffixInfo}
};

#[derive(Debug, Clone, PartialEq)]
pub enum RifGenTarget {
    /// SystemVerilog
    Sv,
    /// Register Abstraction Layer (UVM)
    Ral,
    /// VHDL
    Vhdl,
    /// C Header
    C,
    /// Python Class
    Py,
    /// HTML documentation
    Html,
    /// Latex documentation
    Latex,
    /// Framemaker documentation
    Mif,
    /// SVD (System View Description)
    Svd,
    /// JSON
    Json,
    /// AsciiDoctor
    Adoc,
    /// IP-XACT
    IpXact,
    /// Custom target
    Custom(String),
}

impl From<&str> for RifGenTarget {

    fn from(s: &str) -> Self {
        let s_lc = s.to_lowercase();
        match s_lc.as_ref() {
            "sv" => RifGenTarget::Sv,
            "ral" => RifGenTarget::Ral,
            "vhdl" => RifGenTarget::Vhdl,
            "c" => RifGenTarget::C,
            "py" => RifGenTarget::Py,
            "html" => RifGenTarget::Html,
            "tex" | "latex" => RifGenTarget::Latex,
            "mif" => RifGenTarget::Mif,
            "svd" => RifGenTarget::Svd,
            "json" => RifGenTarget::Json,
            "adoc" => RifGenTarget::Adoc,
            "ipxact" => RifGenTarget::IpXact,
            _ => RifGenTarget::Custom(s_lc),
        }
    }
}

impl std::fmt::Display for RifGenTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RifGenTarget::Sv        => write!(f, "SystemVerilog"),
            RifGenTarget::Ral       => write!(f, "UVM Register Abstraction Layer"),
            RifGenTarget::Vhdl      => write!(f, "VHDL"),
            RifGenTarget::C         => write!(f, "C"),
            RifGenTarget::Py        => write!(f, "Python"),
            RifGenTarget::Html      => write!(f, "HTML"),
            RifGenTarget::Latex     => write!(f, "LaTeX"),
            RifGenTarget::Mif       => write!(f, "Framemaker MIF"),
            RifGenTarget::Svd       => write!(f, "SVD"),
            RifGenTarget::Json      => write!(f, "JSON"),
            RifGenTarget::Adoc      => write!(f, "AsciiDoctor"),
            RifGenTarget::IpXact    => write!(f, "IpXact"),
            RifGenTarget::Custom(n) => write!(f, "'{n}'"),
        }
    }
}

impl<'de> serde::Deserialize<'de> for RifGenTarget {
    fn deserialize<D: serde::Deserializer<'de> >(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(RifGenTarget::from(s.as_ref()))
    }
}

impl RifGenTarget {
    pub fn custom_name(&self) -> Option<String> {
        if let RifGenTarget::Custom(n) = self {
            Some(n.to_owned())
        } else {
            None
        }
    }
}

/// Controls how field limits are used
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum RtlLimit {
    /// Limit is ignored
    Disabled,
    /// Limit is check at hardware level and generate a bus error
    Hardware,
    /// Limit is checked only in simulation with standard assert
    Assert,
    /// Limit is checked only in simulation and use uvm_report_error()
    UvmError,
}

impl FromStr for RtlLimit {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_lc = s.to_lowercase();
        match s_lc.as_ref() {
            "hw" | "hardware" => Ok(RtlLimit::Hardware),
            "sim" | "assert" => Ok(RtlLimit::Assert),
            "uvm" | "uvmerror" | "uvm_error" => Ok(RtlLimit::UvmError),
            _ => Err(format!("Illegal rtl_limit '{s}': expecting hw, sim or uvm"))
        }
    }
}

/// RTL Limit configuration: pair of limit,
/// first one configuring how defined limits are egenrated in hardware (hardware check or simulation assert/error)
/// and the second one is optional and force adding limit to all enum fields
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RtlLimitCfg(pub RtlLimit, pub RtlLimit);

impl RtlLimitCfg {

    /// True when second limit has been defined
    pub fn new(base: Option<RtlLimit>, force: Option<RtlLimit>) -> Self {
        RtlLimitCfg(
            base.unwrap_or(RtlLimit::Hardware),
            force.unwrap_or(RtlLimit::Disabled),
        )
    }

    /// True when second limit has been defined
    pub fn has_force(&self) -> bool {
        self.1 != RtlLimit::Disabled
    }

    /// Return the appropriate limit based on the field definition
    pub fn get(&self, field: &RifFieldInst) -> RtlLimit {
        if field.has_limit() {self.0}
        else {self.1}
    }

    /// True when one of the limit is set to UVM
    pub fn has_uvm(&self) -> bool {
        self.0 == RtlLimit::UvmError ||
        self.1 == RtlLimit::UvmError
    }

}

/// Main configuration structure for YARIG code generation.
///
/// This structure holds all configuration options for parsing RIF files and generating
/// various output targets. It can be created from a TOML configuration file using
/// [`YarigCfg::from_file`], from command-line arguments using [`YarigCfg::from_cli`],
/// or constructed manually.
///
/// # Fields
///
/// ## Input Configuration
/// - `filename`: Path to the main RIF file to compile
/// - `path`: Reference path for resolving relative paths (auto-detected from config file)
/// - `include`: List of directories to search for included RIF files
///
/// ## Generation Control
/// - `targets`: List of output formats to generate (e.g., sv, c, html)
/// - `gen_inc`: List of included components to generate (use `["*"]` for all)
/// - `local`: Components to generate locally (relative to their RIF file location)
/// - `subdir`: Subdirectory name for non-top-level generated files
///
/// ## Global Settings
/// - `public`: Generate only public registers/fields (hide private ones)
/// - `interface`: Override the HDL interface type (e.g., APB, AXI)
/// - `parameters`: Parameter values to override defaults in RIF files
/// - `casing`: Default casing convention for identifiers
/// - `keywords`: Reserved keyword handling configuration
/// - `suffixes`: Suffix definitions for parameterized components
/// - `suffix_rtl_only`: Apply suffixes only to RTL targets
/// - `auto_legacy`: Use legacy order for interrupt registers
///
/// ## Output Configuration
/// - `outputs`: Map of target names to output directories
///
/// ## Target-Specific Settings
/// - `html`, `adoc`, `latex`, `mif`: Documentation generator settings
/// - `c`: C header generator settings
/// - `py`: Python class generator settings
/// - `rtl`: RTL (SystemVerilog/VHDL) generator settings
/// - `ral`: UVM RAL generator settings
/// - `json`, `svd`, `ipxact`: Data format generator settings
///
/// # Example
///
/// ```no_run
/// # use yarig::cfg::YarigCfg;
/// // Load from TOML file
/// let cfg = YarigCfg::from_file("config.toml").unwrap();
///
/// // Parse RIF and generate all configured targets
/// let (component, paths) = cfg.gen_all().unwrap();
/// ```
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct YarigCfg {
    /// File name of the RIF to compile
    pub filename: String,
    /// Path used as reference for relative paths
    pub path: Option<String>,
    /// List of path to search for included reference
    pub include: Vec<String>,
    /// List of included reference to generate (use ["*"] for all)
    pub gen_inc: Vec<String>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
    /// List of included reference which must be generated locally (use ["*"] to match all component in the gen_inc definition)
    pub local: Vec<String>,
    /// List of targets to generate
    pub targets: Vec<RifGenTarget>,
    /// Flag when the output is for public usage (i.e. hide private register/field)
    pub public: bool,
    /// Specify an HDL interface
    pub interface: Option<Interface>,
    /// dictionary of parameters
    pub parameters: HashMap<String,isize>,
    /// dictionary of path associated to each targets
    pub outputs: HashMap<String,String>,
    /// Use legacy order for interrupts (mask before enable)
    pub auto_legacy: bool,
    /// Use suffix only for RTL generation
    pub suffix_rtl_only: bool,
    /// optional suffix definition
    pub suffixes: HashMap<String, SuffixInfo>,
    /// Specify casing used in all targets
    pub casing: Option<Casing>,
    /// Specify reserved keywords
    pub keywords: RsvdKeywordSel,
    //-- Target specific settings--//
    pub html  : CfgHtml,
    pub adoc  : CfgAdoc,
    pub c  : CfgC,
    pub ral: CfgRal,
    pub rtl: CfgRtl,
    pub py : CfgPy,
    pub json : CfgJson,
    pub svd: CfgSvd,
    pub ipxact: CfgIpXact,
    pub mif: CfgMif,
    pub latex: CfgLatex,
}

/// Configuration specific to HTML target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgHtml {
    /// Specify a file for the CSS template
    pub css: Option<String>,
    /// Split the HTML output in multiple files (one perf RIF)
    pub split: Option<bool>,
    /// Casing for register and field name
    pub casing: Option<Casing>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
}

/// Configuration specific to ASCII Doc target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgAdoc {
    /// Split the AsciiDoc output in multiple files (one perf RIF)
    pub split: Option<bool>,
    /// List of included reference to generate (use ["*"] for all): valid only if split is enabled
    pub gen_inc: Option<Vec<String>>,
    /// List of included reference which must be generated locally (use ["*"] to match all component in the gen_inc definition)
    pub local: Option<Vec<String>>,
    /// Casing for register and field name
    pub casing: Option<Casing>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
    /// List of element to skip in the generation
    pub skip: Option<Vec<Skippable>>,
}

/// Configuration specific to LaTeX target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgLatex {
    /// Casing for register and field name
    pub casing: Option<Casing>,
    /// Split the output in multiple files (one per RIF)
    pub split: Option<bool>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
}

/// Configuration specific to C target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgC {
    /// Name of base address offset (default to PERIPH_BASE_ADDR)
    pub base_offset: Option<String>,
    /// List of included reference to generate (use ["*"] for all)
    pub gen_inc: Option<Vec<String>>,
    /// List of included reference which must be generated locally (use ["*"] to match all component in the gen_inc definition)
    pub local: Option<Vec<String>>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
}

/// Configuration specific to RTL target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgRtl {
    /// Number of pipe level for register access (default 1 on the read value)
    pub nb_pipe: Option<u8>,
    /// List of included reference to generate (use ["*"] for all)
    pub gen_inc: Option<Vec<String>>,
    /// List of included reference which must be generated locally (use ["*"] to match all component in the gen_inc definition)
    pub local: Option<Vec<String>>,
    /// Generate constant for register address in the package
    pub const_reg: Option<bool>,
    /// Generate constant for field reset/position/width
    pub const_field: Option<bool>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
    /// Specify how RTL limits are generated
    pub limit: Option<RtlLimit>,
    /// Option to automatically add limits on all enums
    pub force_limit: Option<RtlLimit>,
}

/// Configuration specific to RAL target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgRal {
    /// Name of the register block class (default to uvm_reg_block)
    pub class: Option<String>,
    /// Name of the macro use to instantiate the RAL (default to an internal macro ral_create_reg_block)
    pub macro_name: Option<String>,
    /// List of included reference to generate (use ["*"] for all)
    pub gen_inc: Option<Vec<String>>,
    /// List of included reference which must be generated locally (use ["*"] to match all component in the gen_inc definition)
    pub local: Option<Vec<String>>,
    /// List of optional imports for regsiter block
    pub imports: Option<HashMap<String,String>>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
}

/// Configuration specific to python target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgPy {
    /// Name of python class definition (Peripheral/Register/Field)
    pub class: Option<String>,
    /// Python version
    pub version: Option<PyVersion>,
    /// Create an __init__.py file at the RIF output location
    pub init_file: Option<bool>,
    /// List of included reference to generate (use ["*"] for all)
    pub gen_inc: Option<Vec<String>>,
    /// List of included reference which must be generated locally (use ["*"] to match all component in the gen_inc definition)
    pub local: Option<Vec<String>>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
    /// Directory where regmap.py file is generated
    pub regmap_dir: Option<String>,
}

/// Configuration specific to JSON target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgJson {
    /// List of included reference to generate (use ["*"] for all)
    pub gen_inc: Option<Vec<String>>,
    /// List of included reference which must be generated locally (use ["*"] to match all component in the gen_inc definition)
    pub local: Option<Vec<String>>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
}

/// Configuration specific to SVD target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgSvd {
    /// IP vendor
    pub vendor: Option<String>,
    /// IP version
    pub version: Option<String>,
}

/// Configuration specific to IP Xact target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgIpXact {
    /// IP vendor
    pub vendor: Option<String>,
    /// IP library
    pub library: Option<String>,
    /// IP version
    pub version: Option<String>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
}

/// Configuration specific to MIF target
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct CfgMif {
    /// Split the MIF output in multiple files
    pub split              : Option<bool>,
    /// Style of anchor paragraph
    pub anchor             : Option<String>,
    /// Style of Header level 2
    pub h2                 : Option<String>,
    /// Style of Header level 3
    pub h3                 : Option<String>,
    /// Style of Table title
    pub table_title        : Option<String>,
    /// Style of Table heading
    pub table_heading      : Option<String>,
    /// Style of Table cell
    pub table_cell         : Option<String>,
    /// Style of Table for Rifmux
    pub table_kind_rifmux  : Option<String>,
    /// Style of Table for address mapping
    pub table_kind_mapping : Option<String>,
    /// Style of Table for register definitino
    pub table_kind_reg     : Option<String>,
    /// Dimension of the table for Rifmux, Pages (3 columns)
    pub width_3col     : Option<[f32;3]>,
    /// Dimension of the table for registers (4 columns)
    pub width_4col     : Option<[f32;4]>,
    /// Dimension of the table for fields (5 columns)
    pub width_5col     : Option<[f32;5]>,
    /// List of included reference to generate (use ["*"] for all)
    pub gen_inc: Option<Vec<String>>,
    /// List of included reference which must be generated locally (use ["*"] to match all component in the gen_inc definition)
    pub local: Option<Vec<String>>,
    /// Casing for register and field name
    pub casing: Option<Casing>,
    /// Sub-directory name for generated file which are not the top level
    pub subdir: Option<String>,
}

impl FromStr for YarigCfg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
       toml::from_str(s).map_err(|e| format!("Error parsing configuration: {:?}", e.message()))
    }
}

impl YarigCfg {

    /// Creates a configuration from a TOML file.
    ///
    /// This method reads the TOML configuration file, parses it, and automatically sets
    /// the `path` field to the directory containing the configuration file. This path
    /// is used to resolve relative paths in the configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the TOML configuration file
    ///
    /// # Returns
    ///
    /// Returns `Ok(YarigCfg)` on success, or `Err(String)` with an error message on failure.
    ///
    pub fn from_file<T : Into<PathBuf>>(path: T) -> Result<YarigCfg,String> {
        let path: PathBuf = path.into();
        let s = fs::read_to_string(&path)
            .map_err(|e| format!("Error opening {path:?} : {e:?}"))?;
        let mut cfg = Self::from_str(&s)?;
        if let Some(d) = path.parent() && let Ok(d) = fs::canonicalize(d) {
            cfg.path = d.to_str().map(str::to_string);
        }
        Ok(cfg)
    }

    /// Creates a configuration from command-line arguments.
    ///
    /// If a configuration file is specified in the arguments, it is loaded first, then
    /// any command-line options override the file settings. If no configuration file
    /// is specified, starts with default settings.
    ///
    /// # Arguments
    ///
    /// * `args` - Command-line arguments structure
    ///
    /// # Returns
    ///
    /// Returns `Ok(YarigCfg)` on success, or `Err(String)` with an error message on failure.
    ///
    pub fn from_cli(args: RifGenArgs) -> Result<YarigCfg,String> {
        let mut cfg = if let Some(cfg_path) = &args.cfg {
            YarigCfg::from_file(cfg_path)?
        } else {
            YarigCfg::default()
        };
        cfg.update_with_cli(args);
        Ok(cfg)
    }

    pub fn update_with_cli(&mut self, args: RifGenArgs) {
        if let Some(fname) = args.rif {self.filename = fname.to_owned()};
        if !args.include.is_empty() {self.include = args.include.clone()};
        if !args.gen_inc.is_empty() {self.gen_inc = args.gen_inc.clone()};
        if !args.targets.is_empty() {self.targets = args.targets.to_owned()};
        if args.public {self.public = true};
        if args.auto_legacy {self.auto_legacy = true};
        // Clear target if check is enabled and override target if at least one is defined on the command-line
        if args.check {self.targets.clear();}
        else if !args.targets.is_empty() {self.targets = args.targets.to_owned()};
        //
        if !args.parameters.is_empty() {self.parameters.extend(args.parameters)};
        if let Some(suffix) = args.suffix {self.suffixes.insert("".to_owned(), suffix);};
        if args.py_version.is_some() {self.py.version = args.py_version};
        if args.py_init_file.is_some() {self.py.init_file = args.py_init_file};
        if args.casing.is_some() {self.casing = args.casing};
        if args.subdir.is_some() {self.subdir = args.subdir.clone()};
        if args.interface.is_some() {self.interface = args.interface};
        if args.suffix_rtl_only {self.suffix_rtl_only = true;}
        if args.rtl_const_reg {self.rtl.const_reg = Some(true);}
        if args.rtl_const_field {self.rtl.const_field = Some(true);}
        if let Some(l) = args.rtl_limit {self.rtl.limit = Some(l);}
        if let Some(l) = args.rtl_force_limit {self.rtl.force_limit = Some(l);}
        if args.keyword_rename {self.keywords.error = false;}
        if args.targets.contains(&RifGenTarget::Sv) {self.keywords.sv = true;}
        if args.targets.contains(&RifGenTarget::Vhdl) {self.keywords.vhdl = true;}

        if args.c_base_addr_name.is_some() {
            self.c.base_offset = args.c_base_addr_name;
        }

        if !args.skip.is_empty() {
            self.adoc.skip = Some(args.skip.clone());
        }

        for t in args.split.iter() {
            match t {
                RifGenTarget::Html => self.html.split = Some(true),
                RifGenTarget::Adoc => self.adoc.split = Some(true),
                RifGenTarget::Mif  => self.mif.split  = Some(true),
                _ => eprintln!("Split target {t} not supported ! Expecting html, adoc, mif."),
            }
        }

        //
        let outputs = [
            ("c"  , args.output_c),
            ("py" , args.output_py),
            ("doc", args.output_doc),
            ("json", args.output_json),
            ("rtl", args.output_rtl),
            ("sim", args.output_sim)];
        for (k,v) in outputs.iter() {
            if let Some(path) = v {
                self.outputs.insert(k.to_string(), path.to_owned());
            }
        }
    }

    /// Retrieve output path definition from the configuration given a target
    pub fn get_output_path(&self, target: &RifGenTarget) -> (PathBuf, Option<String>, String) {
        let (keys,def) = match target {
            RifGenTarget::C => (["c", "sw"],"c"),
            RifGenTarget::Html => (["html", "doc"],"doc"),
            RifGenTarget::Mif => (["mif", "doc"], "doc"),
            RifGenTarget::Latex => (["latex", "doc"], "doc"),
            RifGenTarget::Sv => (["sv", "rtl"], "rtl"),
            RifGenTarget::Vhdl => (["vhdl", "rtl"], "rtl"),
            RifGenTarget::Ral => (["ral", "sim"], "sim"),
            RifGenTarget::Py => (["py", "sw"], "py"),
            RifGenTarget::Json => (["json", "doc"], "doc"),
            RifGenTarget::Adoc => (["adoc", "doc"], "doc"),
            RifGenTarget::Svd => (["svd", "sw"], "sw"),
            RifGenTarget::IpXact => (["ipxact", "sw"], "sw"),
            RifGenTarget::Custom(n) => ([n.as_str(),n.as_str()], n.as_str()),
        };
        self.get_output_path_kd(&keys, def)
    }

    /// Retrieve output path definition from the configuration given an array of keys and a default
    pub fn get_output_path_kd(&self, keys: &[&str], def: &str) -> (PathBuf, Option<String>, String) {
        let mut path = format!("./{def}");
        for k in keys.iter() {
            if let Some(p) = self.outputs.get(*k) {
                path = p.to_owned();
                break;
            }
        }
        let is_rel = path.starts_with('.') || !path.contains('/');
        let path_buf : PathBuf = match (&self.path, is_rel) {
            (Some(cwd), true) => [cwd, &path].iter().collect(),
            _ => path.clone().into()
        };

        match (path_buf.extension().is_some(),path_buf.file_name()) {
            (true, Some(name)) => {
                let fname = name.to_string_lossy().to_string();
                let parent = path_buf.parent().unwrap_or(&path_buf).to_path_buf();
                (parent, Some(fname), path)
            }
            _ => (path_buf, None, path)
        }
    }

    pub fn get_local(&self, target: &RifGenTarget) -> Option<&Vec<String>> {
        let local = match target {
            RifGenTarget::C    => self.c.local.as_ref(),
            RifGenTarget::Mif  => self.mif.local.as_ref(),
            RifGenTarget::Sv   |
            RifGenTarget::Vhdl => self.rtl.local.as_ref(),
            RifGenTarget::Ral  => self.ral.local.as_ref(),
            RifGenTarget::Py   => self.py.local.as_ref(),
            RifGenTarget::Adoc => self.adoc.local.as_ref(),
            _ => None
        };
        if local.is_none() {
            Some(&self.local)
        } else {
            local
        }
    }

    pub fn get_subdir(&self, target: &RifGenTarget) -> Option<&String> {
        let subdir = match target {
            RifGenTarget::Html   => self.html.subdir.as_ref(),
            RifGenTarget::Adoc   => self.adoc.subdir.as_ref(),
            RifGenTarget::C      => self.c.subdir.as_ref(),
            RifGenTarget::Ral    => self.ral.subdir.as_ref(),
            RifGenTarget::Sv     |
            RifGenTarget::Vhdl   => self.rtl.subdir.as_ref(),
            RifGenTarget::Py     => self.py.subdir.as_ref(),
            RifGenTarget::Json   => self.json.subdir.as_ref(),
            RifGenTarget::IpXact => self.ipxact.subdir.as_ref(),
            RifGenTarget::Mif    => self.mif.subdir.as_ref(),
            RifGenTarget::Latex  => self.latex.subdir.as_ref(),
            _ => None
        };
        if subdir.is_none() {
            self.subdir.as_ref()
        } else {
            subdir
        }
    }

    /// Parses the RIF file and generates all configured output targets.
    ///
    /// This is the main method that orchestrates the entire generation process:
    /// 1. Resolves the RIF file path (using configured paths and includes)
    /// 2. Parses the RIF file and any included files
    /// 3. Compiles the RIF into an internal representation
    /// 4. Applies parameter overrides and suffix settings
    /// 5. Generates output for each configured target
    ///
    /// # Returns
    ///
    /// Returns a tuple containing:
    /// - `Comp`: The compiled component structure (RIF or RifMux)
    /// - `HashMap<String, PathBuf>`: Map of component names to their file paths
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The RIF file cannot be found or read
    /// - Parsing fails (syntax errors in the RIF file)
    /// - Compilation fails (semantic errors, type mismatches, etc.)
    /// - Any target generator fails to produce output
    ///
    pub fn gen_all(&self) -> Result<(Comp, HashMap<String, PathBuf>), String> {

        let mut params = ParamValues::new();
        self.parameters.iter().for_each(
            |(k,v)| params.insert(k.to_owned(), *v)
        );
        // if !params.is_empty() {println!("Parameters: {params}");}
        let mut rif_path : PathBuf = self.filename.clone().into();
        if !rif_path.exists() && rif_path.is_relative() && let Some(path) = self.path.as_ref() {
            rif_path = [path, &self.filename].iter().collect();
        }
        let parser_cfg = ParserCfg::new(self.keywords, self.auto_legacy);
        let rif_src = RifGenSrc::from_file(&rif_path, &self.include, &parser_cfg)
            .map_err(|e| format!("Parsing Error : {e}"))?;
        let mut rif_obj = Comp::compile(&rif_src, &self.suffixes, &params)
            .map_err(|e| format!("Compilation failed: {e}"))?;
        // Handle case where suffixes are enabled only for RTL targets
        // Force to None by default and will set it properly only in the appropriate target
        let no_suffixes = HashMap::new();
        if self.suffix_rtl_only {
            rif_obj.set_suffixes(&no_suffixes);
        }
        // Force interface if specified in the configuration
        if let Some(intf) = &self.interface {
            rif_obj.set_interface(intf);
        }
        //
        let base_setting = GeneratorBaseSetting::new(self.casing, self.public, &self.gen_inc, &self.subdir);
        for target in self.targets.iter() {
            let mut setting = base_setting.clone();
            let out = self.get_output_path(target);
            setting.set_output((out.0, out.1));
            if let Some(local) = self.get_local(target) {
                setting.set_locals(target, local, &rif_src.paths, &out.2);
            }
            if let Some(subdir) = self.get_subdir(target) {
                setting.subdir = Some(subdir.clone());
            }
            match target {
                RifGenTarget::C => {
                    let mut g = GeneratorC::new(setting, self.c.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("C generation failed: {e}"))?;
                },
                RifGenTarget::Html => {
                    if let Some(split) = self.html.split {setting.split = split;}
                    let mut g = GeneratorHtml::new(setting, self.html.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("Html generation failed: {e}"))?;
                }
                RifGenTarget::Mif => {
                    let mut g = GeneratorMif::new(setting, self.mif.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("MIF generation failed: {e}"))?;
                }
                RifGenTarget::Latex => {
                    if let Some(split) = self.latex.split {setting.split = split;}
                    if let Some(casing) = self.latex.casing {setting.casing = casing;}
                    let mut g = GeneratorLatex::new(setting);
                    g.gen_all(&rif_obj).map_err(|e| format!("Latex generation failed: {e}"))?;
                }
                RifGenTarget::Sv => {
                    if self.suffix_rtl_only {
                        rif_obj.set_suffixes(&self.suffixes);
                    }
                    let mut g = GeneratorSv::new(setting, self.rtl.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("SystemVerilog generation failed: {e}"))?;
                    if self.suffix_rtl_only {
                        rif_obj.set_suffixes(&no_suffixes);
                    }
                }
                RifGenTarget::Vhdl => {
                    if self.suffix_rtl_only {
                        rif_obj.set_suffixes(&self.suffixes);
                    }
                    let mut g = GeneratorVhdl::new(setting, self.rtl.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("VHDL generation failed: {e}"))?;
                    if self.suffix_rtl_only {
                        rif_obj.set_suffixes(&no_suffixes);
                    }
                }
                RifGenTarget::Ral => {
                    let mut g = GeneratorRal::new(setting, self.ral.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("RAL generation failed: {e}"))?;
                }
                RifGenTarget::Py => {
                    let mut g = GeneratorPy::new(setting, self.py.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("Python generation failed: {e}"))?;
                }
                RifGenTarget::Json => {
                    let mut g = GeneratorJson::new(setting);
                    g.gen_all(&rif_obj).map_err(|e| format!("JSON generation failed: {e}"))?;
                }
                RifGenTarget::Adoc => {
                    if let Some(skip) = &self.adoc.skip {
                        setting.skip = skip.clone();
                    }
                    let mut g = GeneratorAdoc::new(setting, self.adoc.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("AsciiDoctor generation failed: {e}"))?;
                }
                RifGenTarget::Svd => {
                    let mut g = GeneratorSvd::new(setting, self.svd.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("SVD generation failed: {e}"))?;
                }
                RifGenTarget::IpXact => {
                    let mut g = GeneratorIpXact::new(setting, self.ipxact.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("IP XACT generation failed: {e}"))?;
                }
                _ => {},
            }
        }
        Ok((rif_obj, rif_src.paths))
    }

}
