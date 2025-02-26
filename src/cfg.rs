use serde_derive::Deserialize;
use clap::{ValueEnum};
use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};
use toml;
use crate::{
	comp::comp_inst::Comp,
	generator::{
		casing::Casing, gen_c::GeneratorC, gen_common::{GeneratorBaseSetting, Privacy}, gen_html::GeneratorHtml, gen_json::GeneratorJson, gen_latex::GeneratorLatex, gen_mif::GeneratorMif, gen_py::GeneratorPy, gen_ral::GeneratorRal, gen_sv::GeneratorSv, trait_doc::GeneratorDoc, trait_sw::GeneratorSw
	},
	parser::{parser_expr::ParamValues, RifGenSrc},
	rifgen::SuffixInfo
};

#[derive(Deserialize, ValueEnum, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RifGenTargets {
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
    Json
}

#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct YarigCfg {
	pub filename: String,
	pub path: Option<String>,
	pub include: Vec<String>,
	pub gen_inc: Vec<String>,
	pub targets: Vec<RifGenTargets>,
	pub public: bool,
	/// dictionary of parameters
	pub parameters: HashMap<String,isize>,
	/// dictionary of path associated to each targets
	pub outputs: HashMap<String,String>,
	/// optional suffix definition
	pub suffixes: HashMap<String, SuffixInfo>,
	//-- Target specific settings--//
	pub c: CfgC,
	pub ral: CfgRal,
	pub rtl: CfgRtl,
	pub py: CfgPy,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CfgC {
	pub base_offset: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CfgRtl {
	pub nb_pipe: u8,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CfgRal {
	pub class: Option<String>,
	pub macro_name: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CfgPy {
	pub class: Option<String>,
}

impl FromStr for YarigCfg {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
       toml::from_str(s).map_err(|e| format!("Error parsing configuration: {:?}", e.message()))
    }
}

impl YarigCfg {

	pub fn from_file<T : Into<PathBuf>>(path: T) -> Result<YarigCfg,String> {
		let path: PathBuf = path.into();
		let s = fs::read_to_string(&path)
			.map_err(|e| format!("Error opening {:?} : {e:?}", path))?;
		let mut cfg = Self::from_str(&s)?;
		if let Some(d) = path.parent() {
			if let Ok(d) = fs::canonicalize(d) {
				cfg.path = d.to_str().map(str::to_string);
			}
		}
		Ok(cfg)
	}

	pub fn get_output_path(&self, keys: &[&str], def: &str) -> PathBuf {
		let mut path = format!("./{def}");
		for k in keys.iter() {
			if let Some(p) = self.outputs.get(*k) {
				path = p.to_owned();
				break;
			}
		}
		let is_rel = path.starts_with('.');
		match (&self.path, is_rel) {
			(Some(cwd), true) => [cwd, &path].iter().collect(),
			_ => path.into()
		}
	}

	pub fn gen_all(&self) -> Result<(), String> {

	    let mut setting = GeneratorBaseSetting {
	        path: "".into(),
	        casing: Casing::Snake,
	        privacy: if self.public {Privacy::Public} else {Privacy::Internal},
	        compact: true,
	        gen_inc: self.gen_inc.clone()
	    };

	    let mut params = ParamValues::new();
	    self.parameters.iter().for_each(
	        |(k,v)| params.insert(k.to_owned(), *v)
	    );
	    // if !params.is_empty() {println!("Parameters: {params}");}

	    let rif_src = RifGenSrc::from_file(&self.filename)?;
	    let rif_obj = Comp::compile(&rif_src, &self.suffixes, &params)
	    	.map_err(|e| format!("Compilation failed: {e}"))?;
	    for target in self.targets.iter() {
	        match target {
	            RifGenTargets::C => {
	                setting.path = self.get_output_path(&["c", "sw"],"c");
	                let mut g = GeneratorC::new(setting.clone(), self.c.base_offset.clone());
	                g.gen_all(&rif_obj).map_err(|e| format!("C generation failed: {e}"))?;
	            },
	            RifGenTargets::Html => {
	                setting.path = self.get_output_path(&["html", "doc"],"doc");
	                let mut g = GeneratorHtml::new(setting.clone());
	                g.gen_all(&rif_obj).map_err(|e| format!("Html generation failed: {e}"))?;
	            }
	            RifGenTargets::Mif => {
	                setting.path = self.get_output_path(&["mif", "doc"], "doc");
	                // TODO: support customization of paragraph style
	                let mut g = GeneratorMif::new(setting.clone());
	                g.gen_all(&rif_obj).map_err(|e| format!("MIF generation failed: {e}"))?;
	            }
	            RifGenTargets::Latex => {
	                setting.path = self.get_output_path(&["latex", "doc"], "doc");
	                let mut g = GeneratorLatex::new(setting.clone());
	                g.gen_all(&rif_obj).map_err(|e| format!("Latex generation failed: {e}"))?;
	            }
	            RifGenTargets::Sv => {
	                setting.path = self.get_output_path(&["sv", "rtl"], "rtl");
	                let mut g = GeneratorSv::new(setting.clone());
	                g.gen_all(&rif_obj).map_err(|e| format!("SystemVerilog generation failed: {e}"))?;
	            }
	            RifGenTargets::Ral => {
	                setting.path = self.get_output_path(&["ral", "sim"], "sim");
	                let mut g = GeneratorRal::new(setting.clone(), self.ral.clone());
	                g.gen_all(&rif_obj).map_err(|e| format!("RAL generation failed: {e}"))?;
	            }
	            RifGenTargets::Py => {
	                setting.path = self.get_output_path(&["py", "sw"], "py");
	                let mut g = GeneratorPy::new(setting.clone(), self.py.clone());
	                g.gen_all(&rif_obj).map_err(|e| format!("Python generation failed: {e}"))?;
	            }
	            RifGenTargets::Json => {
	                setting.path = self.get_output_path(&["json", "doc"], "doc");
	                let mut g = GeneratorJson::new(setting.clone());
	                g.gen_all(&rif_obj).map_err(|e| format!("JSON generation failed: {e}"))?;
	            }
	            t => eprintln!("Target {t:?} not supported -> skipping"),
	        }
	    }
	    Ok(())
	}

}
