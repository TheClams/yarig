use std::{collections::HashMap, path::PathBuf};

use crate::{comp::comp_inst::{Comp, RifFieldInst, RifInst, RifRegInst, RifmuxInst}, parser::remove_rif, rifgen::SuffixInfo};

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

impl GeneratorBaseSetting {
    pub fn is_gen_inc(&self, rif: &RifInst) -> bool {
        let names = [&rif.inst_name, &rif.type_name, &remove_rif(&rif.type_name).to_owned()];
        names.iter().any(|n| self.gen_inc.contains(n))
    }
}

#[derive(Clone, Debug)]
pub struct GeneratorCore {
    /// Basic settings
    pub setting: GeneratorBaseSetting,
    /// Main text buffer
    pub txt: String,
    /// Secondary buffer
    pub stash: Vec<String>,
}

#[allow(dead_code)]
impl GeneratorCore {

    /// Create the core generator structure
    pub fn new(stash_size: usize, setting: GeneratorBaseSetting) -> Self {
        let mut stash = Vec::with_capacity(stash_size);
        // Allocate a small buffer for each stash
        for _ in 0..stash_size {
            stash.push(String::with_capacity(1000));
        }
        GeneratorCore {
            setting,
            txt: String::with_capacity(10000),
            stash,
        }
    }

    /// Write srting on the main text
    pub fn write(&mut self, string: &str) {
        self.txt.push_str(string);
    }

    /// Push string to a stash
    pub fn push_stash(&mut self, idx: usize, string: &str) {
        self.stash[idx].push_str(string);
    }

    /// Pop the content of a stash to the main text
    pub fn pop_stash(&mut self, idx: usize) {
        self.txt.push_str(&self.stash[idx]);
        self.stash[idx].clear();
    }

    /// Pop the content of a stash to another stash
    pub fn pop_stash_to(&mut self, from: usize, to: usize) {
        let mut stash_iter = self.stash.iter_mut();
        let (stash_from, stash_to);
        if from > to {
            stash_to = stash_iter.nth(to).unwrap();
            stash_from = stash_iter.nth(from - to - 1).unwrap();
        } else {
            stash_from = stash_iter.nth(from).unwrap();
            stash_to = stash_iter.nth(to - from - 1).unwrap();
        }
        stash_to.push_str(stash_from);
        stash_from.clear();
    }

    /// Flag when a stash is empty
    pub fn stash_is_empty(&self, idx: usize) -> bool {
        self.stash[idx].is_empty()
    }

    /// Save the main text to a file
    pub fn save(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path : PathBuf = [
            self.setting.path.clone(),
            filename.into()
        ].iter().collect();
        std::fs::write(path, self.txt.as_bytes())?;
        self.txt.clear();
        for s in self.stash.iter_mut() {
            s.clear();
        }
        Ok(())
    }

    /// Get a field name with proper casing
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


pub trait GeneratorBase {

    /// File extension
    const EXT : &'static str;

    /// Get reference to the core generator
    fn core(&self) -> &GeneratorCore;

    /// Get reference to the core generator
    fn core_mut(&mut self) -> &mut GeneratorCore;

    /// Get reference to the core generator
    fn setting(&self) -> &GeneratorBaseSetting {
        &self.core().setting
    }

    /// Write a string in main buffer
    fn write(&mut self, txt: &str) {
        self.core_mut().write(txt);
    }

    /// Save a string in one of the two stash
    fn push_stash(&mut self, idx: usize, txt: &str) {
        self.core_mut().push_stash(idx, txt);
    }

    /// Write a stash content into main buffer and clear the stash
    fn pop_stash(&mut self, idx: usize) {
        self.core_mut().pop_stash(idx);
    }

    /// Write a stash content into main buffer and clear the stash
    fn pop_stash_to(&mut self, from: usize, to: usize) {
        self.core_mut().pop_stash_to(from, to);
    }

    /// Write a stash content into main buffer and clear the stash
    fn stash_is_empty(&mut self, idx: usize) -> bool {
        self.core().stash_is_empty(idx)
    }

    /// Save the main buffer into a file and clear buffer and stash
    fn save(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.core_mut().save(filename)
    }

    /// Write a string in main buffer
    fn casing(&mut self, txt: &str) -> String {
        txt.to_casing(self.setting().casing)
    }

    fn filename_rif(&self, rif: &RifInst) -> String {
        format!("{}.{}", rif.name(false), Self::EXT)
    }

    fn filename_rifmux(&self, rifmux: &RifmuxInst) -> String {
        format!("{}.{}", &rifmux.inst_name, Self::EXT)
    }
}