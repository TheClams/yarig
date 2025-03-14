use serde_derive::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf, str::FromStr};
use toml;
use crate::{
    comp::comp_inst::Comp,
    generator::{
        casing::Casing,
        gen_common::{GeneratorBaseSetting, Privacy},
        trait_doc::GeneratorDoc,
        gen_adoc::GeneratorAdoc,
        gen_html::GeneratorHtml,
        gen_latex::GeneratorLatex,
        gen_mif::GeneratorMif,
        trait_sw::GeneratorSw,
        gen_c::GeneratorC,
        gen_ipxact::GeneratorIpXact,
        gen_json::GeneratorJson,
        gen_py::{GeneratorPy, PyVersion},
        gen_ral::GeneratorRal,
        gen_svd::GeneratorSvd,
        gen_sv::GeneratorSv,
    },
    parser::{parser_expr::ParamValues, RifGenSrc},
    rifgen::{Interface, SuffixInfo}
};

#[derive(Debug, Clone)]
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
    Json,
    /// AsciiDoctor
    Adoc,
    /// IP-XACT
    IpXact,
    /// Custom target
    Custom(String),
}

impl From<&str> for RifGenTargets {

    fn from(s: &str) -> Self {
        let s_lc = s.to_lowercase();
        match s_lc.as_ref() {
            "sv" => RifGenTargets::Sv,
            "ral" => RifGenTargets::Ral,
            "vhdl" => RifGenTargets::Vhdl,
            "c" => RifGenTargets::C,
            "py" => RifGenTargets::Py,
            "html" => RifGenTargets::Html,
            "latex" => RifGenTargets::Latex,
            "mif" => RifGenTargets::Mif,
            "svd" => RifGenTargets::Svd,
            "json" => RifGenTargets::Json,
            "adoc" => RifGenTargets::Adoc,
            _ => RifGenTargets::Custom(s_lc),
        }
    }
}

impl<'de> serde::Deserialize<'de> for RifGenTargets {
    fn deserialize<D: serde::Deserializer<'de> >(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(RifGenTargets::from(s.as_ref()))
    }
}

impl RifGenTargets {
    pub fn custom_name(&self) -> Option<String> {
        if let RifGenTargets::Custom(n) = self {
            Some(n.to_owned())
        } else {
            None
        }
    }
}

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
    /// List of targets to generate
    pub targets: Vec<RifGenTargets>,
    /// Flag when the output is for public usage (i.e. hide private register/field)
    pub public: bool,
    /// Specify an HDL interface
    pub interface: Option<Interface>,
    /// dictionary of parameters
    pub parameters: HashMap<String,isize>,
    /// dictionary of path associated to each targets
    pub outputs: HashMap<String,String>,
    /// Use prefixes only for RTL generation
    pub suffix_rtl_only: bool,
    /// optional suffix definition
    pub suffixes: HashMap<String, SuffixInfo>,
    /// Specify casing used in all targets
    pub casing: Option<Casing>,
    //-- Target specific settings--//
    pub html  : CfgHtml,
    pub c  : CfgC,
    pub ral: CfgRal,
    pub rtl: CfgRtl,
    pub py : CfgPy,
    pub svd: CfgSvd,
    pub ipxact: CfgIpXact,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CfgHtml {
    pub css: Option<String>,
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
    pub version: Option<PyVersion>
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CfgSvd {
    pub vendor: Option<String>,
    pub version: Option<String>,
}

#[derive(Deserialize, Debug, Clone, Default)]
pub struct CfgIpXact {
    pub vendor: Option<String>,
    pub library: Option<String>,
    pub version: Option<String>,
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

    pub fn get_output_path(&self, keys: &[&str], def: &str) -> (PathBuf, Option<String>) {
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
            _ => path.into()
        };

        match (path_buf.extension().is_some(),path_buf.file_name()) {
            (true, Some(name)) => {
                let fname = name.to_string_lossy().to_string();
                let parent = path_buf.parent().unwrap_or(&path_buf).to_path_buf();
                (parent, Some(fname))
            }
            _ => (path_buf, None)
        }
    }

    pub fn gen_all(&self) -> Result<Comp, String> {

        let base_setting = GeneratorBaseSetting {
            path: "".into(),
            fname: None,
            casing: self.casing.unwrap_or(Casing::Snake),
            privacy: if self.public {Privacy::Public} else {Privacy::Internal},
            gen_inc: self.gen_inc.clone()
        };
        let mut params = ParamValues::new();
        self.parameters.iter().for_each(
            |(k,v)| params.insert(k.to_owned(), *v)
        );
        // if !params.is_empty() {println!("Parameters: {params}");}
        let mut rif_path : PathBuf = self.filename.clone().into();
        if !rif_path.exists() && rif_path.is_relative() && self.path.is_some() {
            rif_path = [self.path.as_ref().unwrap(), &self.filename].iter().collect();
        }
        let rif_src = RifGenSrc::from_file(&rif_path, &self.include)
            .map_err(|e| format!("Error opening {:?} : {e:?}", rif_path))?;
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
        for target in self.targets.iter() {
            let mut setting = base_setting.clone();
            match target {
                RifGenTargets::C => {
                    setting.set_output(self.get_output_path(&["c", "sw"],"c"));
                    let mut g = GeneratorC::new(setting, self.c.base_offset.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("C generation failed: {e}"))?;
                },
                RifGenTargets::Html => {
                    setting.set_output(self.get_output_path(&["html", "doc"],"doc"));
                    let mut g = GeneratorHtml::new(setting, self.html.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("Html generation failed: {e}"))?;
                }
                RifGenTargets::Mif => {
                    setting.set_output(self.get_output_path(&["mif", "doc"], "doc"));
                    // TODO: support customization of paragraph style
                    let mut g = GeneratorMif::new(setting);
                    g.gen_all(&rif_obj).map_err(|e| format!("MIF generation failed: {e}"))?;
                }
                RifGenTargets::Latex => {
                    setting.set_output(self.get_output_path(&["latex", "doc"], "doc"));
                    let mut g = GeneratorLatex::new(setting);
                    g.gen_all(&rif_obj).map_err(|e| format!("Latex generation failed: {e}"))?;
                }
                RifGenTargets::Sv => {
                    setting.set_output(self.get_output_path(&["sv", "rtl"], "rtl"));
                    if self.suffix_rtl_only {
                        rif_obj.set_suffixes(&self.suffixes);
                    }
                    let mut g = GeneratorSv::new(setting);
                    g.gen_all(&rif_obj).map_err(|e| format!("SystemVerilog generation failed: {e}"))?;
                    if self.suffix_rtl_only {
                        rif_obj.set_suffixes(&no_suffixes);
                    }
                }
                RifGenTargets::Ral => {
                    setting.set_output(self.get_output_path(&["ral", "sim"], "sim"));
                    let mut g = GeneratorRal::new(setting, self.ral.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("RAL generation failed: {e}"))?;
                }
                RifGenTargets::Py => {
                    setting.set_output(self.get_output_path(&["py", "sw"], "py"));
                    let mut g = GeneratorPy::new(setting, self.py.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("Python generation failed: {e}"))?;
                }
                RifGenTargets::Json => {
                    setting.set_output(self.get_output_path(&["json", "doc"], "doc"));
                    let mut g = GeneratorJson::new(setting);
                    g.gen_all(&rif_obj).map_err(|e| format!("JSON generation failed: {e}"))?;
                }
                RifGenTargets::Adoc => {
                    setting.set_output(self.get_output_path(&["adoc", "doc"], "doc"));
                    let mut g = GeneratorAdoc::new(setting);
                    g.gen_all(&rif_obj).map_err(|e| format!("AsciiDoctor generation failed: {e}"))?;
                }
                RifGenTargets::Svd => {
                    setting.set_output(self.get_output_path(&["svd", "sw"], "sw"));
                    let mut g = GeneratorSvd::new(setting, self.svd.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("SVD generation failed: {e}"))?;
                }
                RifGenTargets::IpXact => {
                    setting.set_output(self.get_output_path(&["ipxact", "sw"], "sw"));
                    let mut g = GeneratorIpXact::new(setting, self.ipxact.clone());
                    g.gen_all(&rif_obj).map_err(|e| format!("IP XACT generation failed: {e}"))?;
                }
                _ => {},
            }
        }
        Ok(rif_obj)
    }

}
