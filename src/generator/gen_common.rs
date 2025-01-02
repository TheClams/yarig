use std::{collections::HashMap, path::PathBuf};

use crate::{comp::comp_inst::{Comp, RifFieldInst, RifInst, RifRegInst, RifmuxInst}, rifgen::SuffixInfo};

use super::casing::{Casing, ToCasing};

pub type InstDict = HashMap<String,Vec<u16>>;

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

#[allow(dead_code)]
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
pub struct GeneratorCore {
    /// Basic settings
    pub setting: GeneratorBaseSetting,
    /// Main text buffer
    pub txt: String,
    /// Secondary buffer
    pub stash: [String; 2],
}

#[allow(dead_code)]
impl GeneratorCore {

    pub fn new(setting: GeneratorBaseSetting) -> Self {
        GeneratorCore {
            setting,
            txt: String::with_capacity(10000),
            stash: [String::with_capacity(1000), String::with_capacity(1000)],
        }
    }

    pub fn write(&mut self, string: &str) {
        self.txt.push_str(string);
    }

    pub fn push_stash(&mut self, idx: usize, string: &str) {
        self.stash[idx].push_str(string);
    }

    pub fn pop_stash(&mut self, idx: usize) {
        self.txt.push_str(&self.stash[idx]);
        self.stash[idx].clear();
    }

    pub fn stash_is_empty(&self, idx: usize) -> bool {
        self.stash[idx].is_empty()
    }

    pub fn save(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path : PathBuf = [
            self.setting.path.clone(),
            filename.into()
        ].iter().collect();
        std::fs::write(path, self.txt.as_bytes())?;
        self.txt.clear();
        self.stash[0].clear();
        self.stash[1].clear();
        Ok(())
    }


    pub fn get_field_name(&self, r: &RifRegInst, f: &RifFieldInst) -> String {
        // println!("[get_field_name] {}.{} : rsvd={}, field array = {:?}, reg array={:?}",
        //     r.reg_name, f.name, f.is_reserved(), f.array, r.array);
        if f.is_reserved() && self.setting.privacy.is_public() {
            format!("rsvd{}",f.lsb)
        } else if f.array.dim() > 1 || r.array.dim()==0 || r.array.is_inst() {
            f.name_flat().to_casing(self.setting.casing)
        } else {
            f.name.to_casing(self.setting.casing)
        }
    }


}
