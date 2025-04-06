use std::{collections::HashMap, ops::Deref, path::PathBuf};

use crate::{
    comp::comp_inst::{Comp, RifFieldInst, RifInst, RifPageInst, RifRegInst, RifmuxInst},
    parser::remove_rif
};

use super::casing::{Casing, ToCasing};

pub struct InstDict(HashMap<String,Vec<u16>>);

impl InstDict {
    pub fn new(pages: &[RifPageInst], is_public: bool) -> Self {
        let mut dict : HashMap<String,Vec<u16>> = HashMap::new();
        for page in pages.iter() {
            for (idx,reg) in page.regs.iter().enumerate().filter(|(_,r)| !(is_public && r.visibility.is_hidden())) {
                let n = reg.expanded_type_name();
                dict.entry(n).or_default().push(idx as u16);
            }
        }
        InstDict(dict)
    }

    pub fn first_inst<'a>(&self, page: &'a RifPageInst, reg: &RifRegInst) -> &'a RifRegInst {
        let reg_decl_idx = self
            .get(&reg.expanded_type_name())
            .expect("all register type should be collected !")
            .first().expect("register instance list should not be empty !");
        page.regs
            .get(*reg_decl_idx as usize)
            .expect("register index should exist in the page !")
    }
}

impl Deref for InstDict {
    type Target = HashMap<String,Vec<u16>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub type RifInstInfo = (u64, String, String);

pub struct RifList<'a>(Vec<(&'a RifInst,Vec<RifInstInfo>)>);

impl<'a> RifList<'a> {

    pub fn new(rifmux: &'a RifmuxInst, deep: bool) -> Self {
        let mut rd = RifList(Vec::with_capacity(rifmux.components.len()));
        rd.scan(rifmux, deep, 0);
        rd
    }

    pub fn scan(&mut self, rifmux: &'a RifmuxInst, deep: bool, base_addr: u64) {
        for comp in rifmux.components.iter() {
            let addr = base_addr + comp.full_addr(&rifmux.groups);
            match &comp.inst {
                Comp::Rifmux(c) => if deep {
                    self.scan(c, true, addr)
                },
                Comp::Rif(c) => {
                    let info = (addr, c.inst_name.to_owned(), c.description.get_short().to_owned());
                    if let Some(ri) = self.0.iter_mut().find(|x| x.0.type_name==c.type_name) {
                        ri.1.push(info);
                    } else {
                        self.0.push((c, vec![info]));
                    }
                }
                Comp::External(_) => {}
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item=&(&RifInst,Vec<RifInstInfo>)> {
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
    pub path: PathBuf,
    /// Top filename
    pub fname: Option<String>,
    /// Casing used on register/field
    pub casing: Casing,
    /// Confidentality: confidential or public
    pub privacy: Privacy,
    /// List of included component to generate
    pub gen_inc: Vec<String>,
}

impl GeneratorBaseSetting {

    pub fn set_output(&mut self, info: (PathBuf, Option<String>) ) {
        self.path = info.0;
        self.fname = info.1;
    }

    pub fn is_gen_inc(&self, rif: &RifInst) -> bool {
        let names = [&rif.inst_name, &rif.type_name, &remove_rif(&rif.type_name).to_owned()];
        names.iter().any(|n| self.gen_inc.contains(n))
    }

    pub fn is_gen_all(&self) -> bool {
        self.gen_inc.first().map(|c| c.as_str())==Some("*")
    }
}

/// Component basic information: name, addr/data bus width, page/group number
pub struct CompInfo {
    pub name: String,
    pub addr_width: u8,
    pub data_width: u8,
    pub cnt: usize,
}

impl From<&RifInst> for CompInfo {
    fn from(rif: &RifInst) -> Self {
        CompInfo {
            name: rif.type_name.to_owned(),
            addr_width: rif.addr_width,
            data_width: rif.data_width,
            cnt: rif.pages.len()
        }
    }
}

impl From<&RifmuxInst> for CompInfo {
    fn from(rifmux: &RifmuxInst) -> Self {
        CompInfo {
            name: rifmux.type_name.to_owned(),
            addr_width: rifmux.addr_width,
            data_width: rifmux.data_width,
            cnt: rifmux.groups.len()
        }
    }
}

impl From<&Comp> for CompInfo {
    fn from(comp: &Comp) -> Self {
        let cnt = match comp {
            Comp::Rifmux(rifmux) => rifmux.groups.len(),
            Comp::Rif(rif) => rif.pages.len(),
            Comp::External(_) => 0,
        };
        CompInfo {
            name: comp.get_name().to_owned(),
            addr_width: comp.get_addr_width(),
            data_width: comp.get_data_width(),
            cnt
        }
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
    pub fn get_field_name(&self, reg: &RifRegInst, field: &RifFieldInst) -> String {
        // println!("[get_field_name] {}.{} : rsvd={}, field array = {:?}, reg array={:?}",
        //     reg.reg_name, f.name, f.is_reserved(), f.array, reg.array);
        if field.is_reserved() && self.setting.privacy.is_public() {
            format!("rsvd{}",field.lsb)
        } else if field.array.dim() > 1 || reg.array.dim()==0 || reg.array.is_inst() {
            field.name_flat().to_casing(self.setting.casing)
        } else {
            field.name.to_casing(self.setting.casing)
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