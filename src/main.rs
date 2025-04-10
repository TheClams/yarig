
use std::error::Error;
use clap::Parser;
use yarig::{
    cfg::{RifGenTargets, YarigCfg},
    generator::{casing::Casing, gen_py::PyVersion},
    rifgen::{Interface, SuffixInfo}
};

// use crate::comp::comp_inst::RifmuxMap;

#[derive(Parser)]
#[command(version, rename_all="snake_case")]
/// Register Interface Generator
struct RifGenArgs{
    /// path to the RIF file to parse
    #[arg(short, long)]
    rif: Option<String>,
    /// path to a config file
    #[arg(short, long)]
    cfg: Option<String>,
    /// path to the RIF file to parse
    #[arg(short, long)]
    include: Vec<String>,
    /// List of targets
    #[arg(short, long, num_args = 1..)]
    targets: Vec<RifGenTargets>,
    /// List of included component to generate. Use "*" to select all.
    #[arg(long, num_args = 0..)]
    gen_inc: Vec<String>,
    /// Output path for C header
    #[arg(long)]
    output_c: Option<String>,
    /// Output path for Python classes
    #[arg(long)]
    output_py: Option<String>,
    /// Output path for documentation output (HTML, latex, ...)
    #[arg(long)]
    output_doc: Option<String>,
    /// Output path for JSON output
    #[arg(long)]
    output_json: Option<String>,
    /// Output path for hardware output (SV, VHDL)
    #[arg(long)]
    output_rtl: Option<String>,
    /// Output path for simulation output (RAL)
    #[arg(long)]
    output_sim: Option<String>,
    /// Public documentation (hide all private registers/fields)
    #[arg(long, action)]
    public: bool,
    /// Set parameters value
    #[arg(short = 'P', value_parser = parse_key_val::<String, isize>)]
    parameters: Vec<(String, isize)>,
    /// Set suffix value
    #[arg(short = 'S', long)]
    suffix: Option<SuffixInfo>,
    /// List of target which should split their output. Supported targets are: html, mif, adoc
    #[arg(long, num_args = 0..)]
    split: Vec<RifGenTargets>,
    /// Rename field using reserved keyword
    #[arg(long, action)]
    keyword_rename: bool,
    /// Use suffix only  for RTL outputs
    #[arg(long, action)]
    suffix_rtl_only: bool,
    /// Specify an HDL interface
    #[arg(long)]
    interface: Option<Interface>,
    /// Specify casing used in all targets
    #[arg(long)]
    casing: Option<Casing>,
    /// C macro name defining the base address of the top level
    #[arg(long)]
    c_base_addr_name: Option<String>,
    /// Base class for python target
    #[arg(long)]
    py_class: Option<String>,
    /// Python version (default 3.11)
    #[arg(long)]
    py_version: Option<PyVersion>,
    /// Base class for RAL target
    #[arg(long)]
    ral_class: Option<String>,
    /// Name of macro to create RAL register block
    #[arg(long)]
    ral_macro: Option<String>,
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


fn main() {

    let args = RifGenArgs::parse();

    let mut cfg =
        if let Some(cfg_path) = args.cfg {
            match YarigCfg::from_file(cfg_path) {
                Ok(cfg) => cfg,
                Err(msg) => {
                    eprintln!("{msg}");
                    return;
                },
            }
        } else {
            YarigCfg::default()
        };
    // println!("cfg = {cfg:#?}");

    // Update configuration with command line arguments
    if let Some(fname) = args.rif {cfg.filename = fname.to_owned()};
    if cfg.filename.is_empty() {
        eprintln!("A RIF file must be specified ! (argument -r/--rif");
        return;
    }
    if !args.include.is_empty() {cfg.include = args.include.clone()};
    if !args.gen_inc.is_empty() {cfg.gen_inc = args.gen_inc.clone()};
    if !args.targets.is_empty() {cfg.targets = args.targets.to_owned()};
    if args.public {cfg.public = true};
    if !args.targets.is_empty() {cfg.targets = args.targets.to_owned()};
    if !args.parameters.is_empty() {cfg.parameters.extend(args.parameters)};
    if let Some(suffix) = args.suffix {cfg.suffixes.insert("".to_owned(), suffix);};
    if args.py_version.is_some() {cfg.py.version = args.py_version};
    if args.casing.is_some() {cfg.casing = args.casing};
    if args.interface.is_some() {cfg.interface = args.interface};
    if args.suffix_rtl_only {cfg.suffix_rtl_only = true;}
    if args.keyword_rename {cfg.keywords.error = false;}
    if args.targets.contains(&RifGenTargets::Sv) {cfg.keywords.sv = true;}
    if args.targets.contains(&RifGenTargets::Vhdl) {cfg.keywords.vhdl = true;}

    for t in args.split.iter() {
        match t {
            RifGenTargets::Html => cfg.html.split = Some(true),
            RifGenTargets::Adoc => cfg.adoc.split = Some(true),
            RifGenTargets::Mif  => cfg.mif.split  = Some(true),
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
            cfg.outputs.insert(k.to_string(), path.to_owned());
        }
    }

    if let Err(e) = cfg.gen_all() {
        eprintln!(" -> Error ! {e}");
    }

}
