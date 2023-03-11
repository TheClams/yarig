use std::path::PathBuf;

use crate::{comp::comp_inst::{Comp, RifInst, RifmuxInst}, rifgen::SuffixInfo};

use super::casing::Casing;

pub struct RifList<'a>(Vec<&'a RifInst>);

impl<'a> RifList<'a> {

    pub fn new(rifmux: &'a RifmuxInst) -> Self {
        let mut rd = RifList(Vec::with_capacity(rifmux.components.len()));
        rd.scan(rifmux);
        rd
    }

    pub fn scan(&mut self, rifmux: &'a RifmuxInst) {
        for comp in rifmux.components.iter() {
            match &comp.inst {
                Comp::Rifmux(c) => self.scan(c),
                Comp::Rif(c) =>
                    if !self.0.iter().any(|x| x.type_name==c.type_name) {
                        self.0.push(c);
                    }
                Comp::External(_) => {}
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=&&RifInst> {
        self.0.iter()
    }
}


#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Privacy {#[default]
    /// Show all information including hidden/reserved
    Internal,
    /// All hidden information is skipped, while reserved field/register
    /// have their name and documentation replaced by generic name/comments
    Public
}

#[allow(dead_code)]
impl Privacy {
    pub fn is_public(&self) -> bool {
        *self==Privacy::Public
    }
    pub fn is_internal(&self) -> bool {
        *self==Privacy::Internal
    }
}

#[derive(Clone, Debug)]
pub struct GeneratorBaseSetting {
    /// Output directory path
    pub path: String,
    /// Path to a template file for he generator
    pub template: String,
    /// Suffix to add on the filename
    pub suffix: SuffixInfo,
    /// Casing used on register/field
    pub casing: Casing,
    /// Confidentality: confidential or public
    pub privacy: Privacy,
    /// Document only option: true for compact view
    pub compact: bool,
    /// List of included component to generate
    pub gen_inc: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct GeneratorBase {
    /// Basic settings
    pub setting: GeneratorBaseSetting,
    /// Main text buffer
    pub txt: String,
    /// Secondary buffer
    pub stash: String,
}

#[allow(dead_code)]
impl GeneratorBase {

    pub fn new(setting: GeneratorBaseSetting) -> Self {
        GeneratorBase {
            setting,
            txt: String::with_capacity(10000),
            stash: String::with_capacity(1000)
        }
    }

    fn write(&mut self, string: &str) {
        self.txt.push_str(string);
    }

    fn push_stash(&mut self, string: &str) {
        self.stash.push_str(string);
    }

    fn pop_stash(&mut self) {
        self.txt.push_str(&self.stash);
        self.stash.clear();
    }

    fn save(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path : PathBuf = [
            self.setting.path.clone(),
            filename.into()
        ].iter().collect();
        std::fs::write(path, self.txt.as_bytes())?;
        Ok(())
    }

}
