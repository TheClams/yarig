mod error;
mod parser;
mod rifgen;
mod comp;
mod generator;

use std::{collections::HashMap, error::Error, fs, path::PathBuf};
use clap::{Parser, ValueEnum};
use generator::{
    casing::Casing, gen_c::GeneratorC, gen_common::{GeneratorBaseSetting, Privacy}, gen_html::GeneratorHtml, gen_sv::GeneratorSv
};
use parser::parser_expr::ParamValues;
use rifgen::SuffixInfo;

use comp::comp_inst::Comp;

// use crate::comp::comp_inst::RifmuxMap;

#[derive(Parser)]
#[command(version, rename_all="snake_case")]
/// Register Interface Generator
struct RifGenArgs{
    /// path to the RIF file to parse
    #[arg(short, long, default_value_t = String::from("e:/work/shared/rif_test/rif"))]
    rif: String,
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
    /// C macro name defining the base address of the top level
    #[arg(long, default_value_t = String::from("PERIPH_BASE_ADDR"))]
    c_base_addr_name: String,
    /// Output path for documentation output (HTML, latex, ...)
    #[arg(long, default_value_t = String::from("doc"))]
    output_doc: String,
    /// Output path for documentation output (HTML, latex, ...)
    #[arg(long, default_value_t = String::from("rtl"))]
    output_rtl: String,
    /// Public documentation (hide all private registers/fields)
    #[arg(long, action)]
    public: bool,
    /// Set parameters value
    #[arg(short = 'P', value_parser = parse_key_val::<String, isize>)]
    parameters: Vec<(String, isize)>,
    /// Set suffix value
    // #[arg(short = 'S', value_parser = parse_key_val::<String, isize>)]
    #[arg(short = 'S', long)]
    suffix: Option<SuffixInfo>,
}

#[derive(ValueEnum, Debug, Clone)]
enum RifGenTargets {
    Sv, Vhdl, C, Html, Py, Svd, Json
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
    let rif_path : PathBuf = args.rif.into();

    let filelist: Vec<PathBuf> =
        if rif_path.is_dir() {
            fs::read_dir(rif_path)
                .unwrap()
                .filter(|p| p.as_ref().unwrap().path().extension().map(|s| s=="rif").unwrap_or(false))
                .map(|p| p.unwrap().path())
                .collect()
        }
        else {
            vec![rif_path]
        };

    let mut setting = GeneratorBaseSetting {
        path: "doc".to_owned(),
        template: "".to_owned(),
        suffix: SuffixInfo::new("".to_owned(),false,false),
        casing: Casing::Snake,
        privacy: if args.public {Privacy::Public} else {Privacy::Internal},
        compact: true,
        gen_inc: args.gen_inc
    };

    // println!("{:?}", filelist);

    let mut params = ParamValues::new();
    args.parameters.iter().for_each(
        |(k,v)| params.insert(k.to_owned(), *v)
    );
    if !params.is_empty() {println!("Parameters: {params}");}

    let mut suffixes : HashMap<String, SuffixInfo> = HashMap::new();
    if let Some(suffix) = args.suffix {
        suffixes.insert("".to_owned(), suffix);
    }

    let mut fail_cnt = 0;
    for f in &filelist {
        println!("Parsing of {:?}", f.as_path());
        let p = parser::RifGenSrc::from_file(f);
        match p {
            Ok(rif_src) => {
                println!(" -> Parsing Successful");
                // println!("Rifs compiles = {:?}", rif_src.rifs.keys().join(", "));
                let obj = Comp::compile(&rif_src, &suffixes, &params);
                match &obj {
                    Ok(o) => {
                        println!("   => Compile Ok");
                        for target in args.targets.iter() {
                            match target {
                                RifGenTargets::C => {
                                    setting.path = args.output_c.clone();
                                    let mut gen = GeneratorC::new(setting.clone(), args.c_base_addr_name.to_owned());
                                    if let Err(e) = gen.gen(o) {
                                        println!(" -> C generation failed: {}", e)
                                    }
                                },
                                RifGenTargets::Html => {
                                    setting.path = args.output_doc.clone();
                                    let mut gen = GeneratorHtml::new(setting.clone());
                                    if let Err(e) = gen.gen(o) {
                                        println!(" -> HTML generation failed: {}", e)
                                    }
                                }
                                RifGenTargets::Sv => {
                                    setting.path = args.output_rtl.clone();
                                    let mut gen = GeneratorSv::new(setting.clone());
                                    if let Err(e) = gen.gen(o) {
                                        println!(" -> SV generation failed: {}", e)
                                    }
                                }
                                t => println!("Target {t:?} not supported -> skipping"),
                            }
                        }
                        // println!(" -> Compile Ok: \n{:?}",o),
                    }
                    Err(e) => {fail_cnt+=1; println!(" -> Compile failed: {}", e)},
                }
            },
            // Ok(r) => println!("Parsing of {f} successful :\n {:#?}",r),
            Err(e) => {fail_cnt+=1; println!(" -> {}", e)},
        }
    }
    if fail_cnt > 0 {
        println!("Failed {}/{}",fail_cnt,filelist.len());
    }
}
