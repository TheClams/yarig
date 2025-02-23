
use std::error::Error;
use clap::Parser;
use yarig::{
    cfg::{YarigCfg, RifGenTargets},
    rifgen::SuffixInfo
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
    #[arg(long, num_args = 0..)]
    gen_inc: Vec<String>,
    /// Output path for C header
    #[arg(long, default_value_t = String::from("c"))]
    output_c: String,
    /// Output path for Python classes
    #[arg(long, default_value_t = String::from("py"))]
    output_py: String,
    /// Output path for documentation output (HTML, latex, ...)
    #[arg(long, default_value_t = String::from("doc"))]
    output_doc: String,
    /// Output path for hardware output (SV, VHDL)
    #[arg(long, default_value_t = String::from("rtl"))]
    output_rtl: String,
    /// Output path for simulation output (RAL)
    #[arg(long, default_value_t = String::from("sim"))]
    output_sim: String,
    /// Public documentation (hide all private registers/fields)
    #[arg(long, action)]
    public: bool,
    /// Set parameters value
    #[arg(short = 'P', value_parser = parse_key_val::<String, isize>)]
    parameters: Vec<(String, isize)>,
    /// Set suffix value
    #[arg(short = 'S', long)]
    suffix: Option<SuffixInfo>,
    /// C macro name defining the base address of the top level
    #[arg(long)]
    c_base_addr_name: Option<String>,
    /// Base class for python target
    #[arg(long)]
    py_class: Option<String>,
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
    if !args.include.is_empty() {cfg.include = args.include.clone()};
    if !args.gen_inc.is_empty() {cfg.gen_inc = args.gen_inc.clone()};
    if !args.targets.is_empty() {cfg.targets = args.targets.to_owned()};
    if args.public {cfg.public = true};
    if !args.targets.is_empty() {cfg.targets = args.targets.to_owned()};
    if !args.parameters.is_empty() {cfg.parameters.extend(args.parameters)};
    if let Some(suffix) = args.suffix {cfg.suffixes.insert("".to_owned(), suffix);};

    if let Err(e) = cfg.gen_all() {
        eprintln!(" -> Error ! {e}");
    }

}
