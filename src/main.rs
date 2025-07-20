use clap::Parser;
use yarig::{
    cfg::YarigCfg,
    cli::RifGenArgs}
;

fn main() {

    let args = RifGenArgs::parse();

    let cfg;
    match YarigCfg::from_cli(args) {
        Ok(c) => cfg = c,
        Err(msg) => {
            eprintln!("{msg}");
            return;
        }
    }

    if cfg.filename.is_empty() {
        eprintln!("A RIF file must be specified ! (argument -r/--rif or through a command .toml file)");
        return;
    }

    if let Err(e) = cfg.gen_all() {
        eprintln!(" -> Error ! {e}");
    } else if cfg.targets.is_empty() {
        println!("Compilation succeed !")
    }

}
