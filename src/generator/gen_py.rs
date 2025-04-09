use std::{collections::{BTreeMap, BTreeSet}, fs::File, io::Write, str::FromStr};

use crate::{
    cfg::CfgPy,
    comp::comp_inst::{RifFieldInst, RifInst, RifPageInst, RifRegInst, RifmuxInst},
    parser::remove_rif,
    rifgen::{Description, EnumDef, EnumEntry}
};

use super::{
    casing::{Casing, ToCasing},
    gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore, InstDict, RifList},
    trait_sw::{GeneratorSw, RifContext}
};

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub enum PyVersion {
    V3_10,
    V3_11,
    V3Recent
}

impl std::str::FromStr for PyVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut v = s.split('.');
        let (Some(major),Some(minor)) = (v.next(), v.next()) else {
            return Err("Expecting python version like '3.11'".to_owned())
        };
        let major : u8 = major.parse().map_err(|_| "Invalid major version number".to_owned())?;
        let minor : u8 = minor.parse().map_err(|_| "Invalid minor version number".to_owned())?;
        match (major,minor) {
            (3,10) => Ok(PyVersion::V3_10),
            (3,11) => Ok(PyVersion::V3_11),
            (3,m) if m > 11 => Ok(PyVersion::V3Recent),
            _ => Err("Python version not supported: expecting 3.10+".to_owned())
        }
    }
}

impl<'de> serde::Deserialize<'de> for PyVersion {
    fn deserialize<D: serde::Deserializer<'de> >(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        PyVersion::from_str(&s).map_err(serde::de::Error::custom)
    }
}


pub struct GeneratorPy {
    /// Base structure of all generators
    core: GeneratorCore,
    /// Base python module defining Peripheral, Register and Field classes
    base_module : String,
    /// Python version
    version : PyVersion,
    /// Current Component name (Rifmux or rif)
    comp_name : String,
    /// Current RIF data bus width
    data_width : u8,
    /// Flag when current RIF has enum definition
    has_enum : bool,
    /// Dictionary containing the register owning a field array definition
    field_parent : BTreeMap<String,String>,
    /// Dictionary containing the list of field array in a register
    field_array : BTreeSet<String>,
}


impl GeneratorPy {
    const DEFAULT_BASECLASS : &'static str = include_str!("resources/regmap.py");

    pub fn new(setting: GeneratorBaseSetting, extra: CfgPy) -> Self {
        let mut core = GeneratorCore::new(2,setting);
        // Override gen_inc if defined in the python settings
        if let Some(gen_inc) = extra.gen_inc {
            core.setting.gen_inc = gen_inc;
        }
        GeneratorPy {
            core,
            comp_name: "".to_owned(),
            version: extra.version.unwrap_or(PyVersion::V3_11),
            data_width: 32,
            has_enum: false,
            base_module: extra.class.unwrap_or(".regmap".to_owned()),
            field_parent: BTreeMap::new(),
            field_array: BTreeSet::new(),
        }
    }

    fn desc_to_string(&mut self, desc: &Description, lvl: usize) -> Option<String> {
        let (desc_short, desc_details) = desc.get_split();
        if desc_short.is_empty() {
            return None;
        }
        let indent = "   ".repeat(lvl);
        let mut s = format!("{indent}'''{desc_short}");
        if let Some(desc_details) = desc_details {
            s.push_str(&format!("\n\n{indent}"));
            let mut desc_lines = desc_details.lines().peekable();
            while let Some(l) = desc_lines.next() {
                s.push_str(&l.replace('\\',"").replace('\'', "\\\'"));
                if desc_lines.peek().is_some() {
                    s.push_str("\n         ");
                }
            }
        }
        s.push_str("'''\n");
        Some(s)
    }

}

impl GeneratorBase for GeneratorPy {

    const EXT : &'static str = "py";

    fn core(&self) -> &GeneratorCore {
        &self.core
    }

    fn core_mut(&mut self) -> &mut GeneratorCore {
        &mut self.core
    }

}


impl GeneratorSw for GeneratorPy {
    const HAS_ENUM        : bool = true;
    const SINGLE_FILE     : bool = false;
    const HAS_UNUSED      : bool = false;
    const INST_BY_PAGE    : bool = false;
    const INC_PAGENAME    : bool = false;
    const INST_ARRAY      : bool = false;
    const IS_HIERARCHICAL : bool = false;

    fn set_rif_info(&mut self, rif: &RifInst) {
        self.comp_name = rif.type_name.to_owned().to_lowercase();
        self.data_width = rif.data_width;
        self.has_enum = rif.enum_defs.iter().any(|d| !d.name.starts_with("doc:"))
    }

    /// Create the regmap.py containing base class if not defined in another python module
    fn create_resource(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.base_module==".regmap" {
            let path : std::path::PathBuf = [
                self.setting().path.clone(),
                "regmap.py".into()
            ].iter().collect();
            match self.version {
                PyVersion::V3_10 => {
                    let mut file = File::create(path)?;
                    for l in Self::DEFAULT_BASECLASS.lines() {
                        if l=="    @typing.override" { continue; }
                        file.write_all(l.as_bytes())?;
                        file.write_all(b"\n")?;
                    }
                }
                _ => std::fs::write(path, Self::DEFAULT_BASECLASS.as_bytes())?,
            }
        }
        Ok(())
    }

    fn write_rif_header(&mut self, rif: &RifInst, _base_addr: Option<u64>) {
        let rif_name = remove_rif(&self.comp_name).to_casing(Casing::Pascal);
        self.write("from typing import final\n");
        if self.has_enum {
            self.write("from enum import IntEnum\n");
        }
        self.write(&format!("from {} import Field, Register, Peripheral\n\n", self.base_module));
        self.write("@final\n");
        self.write(&format!("class {rif_name}(Peripheral):\n"));
        if let Some(desc) = self.desc_to_string(&rif.description,1) {
            self.write(&desc);
        }
        self.write("\n");
        // Cleanup field parent dictionnary
        self.field_parent.clear();
    }

    /// Write enum start of declaration statement
    fn write_enum_header(&mut self, type_name: &str, desc: &str) {
        self.write(&format!("   class e_{type_name}(IntEnum):\n"));
        self.write(&format!("      '''{desc}'''\n"));
    }

    /// Write enum entry
    fn write_enum_entry(&mut self, entry: &EnumEntry , is_last: bool) {
        self.write(&format!("      {} = {}\n", entry.name, entry.value));
        if let Some(desc) = self.desc_to_string(&entry.description,2) {
            self.write(&desc);
        }
        if is_last {
            self.write("\n");
        }
    }

    fn write_reg_header(&mut self, _basename: &str, reg: &RifRegInst) {
        let typename = reg.reg_type.to_casing(Casing::Pascal);
        self.write("   @final\n");
        self.write(&format!("   class {typename}(Register):\n"));
        if let Some(desc) = self.desc_to_string(&reg.base_description,2) {
            self.write(&desc);
        }
        //
        let mut flags = Vec::new();
        if reg.is_external() {
            flags.push("external");
        }
        if reg.is_intr() {
            flags.push("interrupt");
        }
        if !flags.is_empty() {
            self.push_stash(0, &format!("         self.__reg_info__.flags = {flags:?}\n"));
        }
        // Cleanup field array dictionnary
        self.field_array.clear();
    }

    // End of register declaration: add the init function with all the field instance
    fn write_reg_footer(&mut self, _basename: &str, reg: &RifRegInst, _is_last: bool) {
        self.write("\n      def __init__(self, parent: None | Peripheral, name: str, addr: int, init: None|int = None) :\n");
        let ro = if reg.sw_access.is_writable() {"False"} else {"True"};
        self.write(&format!("         super().__init__(parent, name, addr, {ro}, init)\n"));
        self.pop_stash(0);
        self.write("\n");
    }

    fn write_field_decl(&mut self, _basename: &str, reg: &RifRegInst, field: &RifFieldInst, enum_def: Option<&EnumDef>, _is_last: bool) {
        let field_type = field.name.to_casing(Casing::Pascal);
        let reg_type = reg.reg_type.to_casing(Casing::Pascal);
        let name = field.name.to_casing(Casing::Snake);
        let idx = if field.array.dim() > 1 {format!("[{}]", field.array.idx())} else {"".to_owned()};
        //
        if field.array.dim() > 0 && !self.field_array.contains(&field.name) {
            if field.array.dim() > 1 {
                self.push_stash(0, &format!("         self.{name} : dict [int, {}.{}.{field_type}] = {{}}\n",
                    remove_rif(&self.comp_name).to_casing(Casing::Pascal),
                    self.field_parent.get(&field.name).unwrap_or(&reg_type),
                ));
            }
            self.field_array.insert(field.name.clone());
            if field.array.idx() == 0 { // Need to handle out-of-order field array
                self.field_parent.insert(field.name.clone(), reg_type.to_owned());
            }
        }
        self.push_stash(0, &format!("         self.{name}{idx} = {}.{}.{field_type}(self, '{name}{idx}')\n",
            remove_rif(&self.comp_name).to_casing(Casing::Pascal),
            self.field_parent.get(&field.name).unwrap_or(&reg_type),
        ));
        // Doc-string
        if let Some(desc) = self.desc_to_string(&field.description,3) {
            self.push_stash(0, &desc);
        }
        // Skip the Field type declaration if already defined
        if field.array.idx() > 0 {
            return;
        }
        // Class declaration
        self.write(&format!("\n      class {field_type}(Field):\n"));
        if let Some(desc) = self.desc_to_string(&field.base_description,3) {
            self.write(&desc);
        }
        //
        let tab = " ".repeat(9);
        self.write(&format!("{tab}pos : int = {}\n", field.lsb));
        self.write(&format!("{tab}width : int = {}\n", field.width));
        self.write(&format!("{tab}value : int = {}\n", field.reset()));
        self.write(&format!("{tab}signed : bool = {}\n",
            if field.is_signed() {"True"} else {"False"}));
        if field.nb_frac != 0 {
            self.write(&format!("{tab}nb_frac : int = {}\n", field.nb_frac));
        }
        self.write(&format!("{tab}kind : str = {:?}\n", field.sw_kind.access_str()));
        if let Some(def) = enum_def.filter(|d| !d.name.starts_with("doc:")) {
            if self.version > PyVersion::V3_10 {
                self.write(&format!("\n{tab}@typing.override"));
            }
            let comp = remove_rif(&self.comp_name).to_casing(Casing::Pascal);
            self.write(&format!("\n{tab}def enum_kind(self) -> None| type[IntEnum]:\n"));
            self.write(&format!("{tab}   return {comp}.{}\n", def.name));
        }
    }

    fn write_page_footer(&mut self, _name: &str, _is_last: bool) {
        self.write("\n   def __init__(self, name: str, addr: int):\n");
        self.write("      super().__init__(name, addr)\n");
        self.pop_stash(1)
    }

    fn write_reginst(&mut self, _basename: &str, page: &RifPageInst, reg: &RifRegInst, _inst_dict: &InstDict, _is_last: bool) {
        let name = reg.reg_name.to_casing(Casing::Snake);
        if reg.array.dim() > 1 {
            if reg.array.idx() == 0 {
                self.push_stash(1, &format!("      self.{name} : list[{}.{}] = []\n",
                    remove_rif(&self.comp_name).to_casing(Casing::Pascal),
                    reg.reg_type.to_casing(Casing::Pascal),
                ));
                if let Some(desc) = self.desc_to_string(&reg.base_description,2) {
                    self.push_stash(1, &desc);
                }
            }
            self.push_stash(1, &format!("      self.{name}.append({}.{}(self, \"{name}{}\", {}, {:#x}))\n",
                remove_rif(&self.comp_name).to_casing(Casing::Pascal),
                reg.reg_type.to_casing(Casing::Pascal),
                reg.array.idx(),
                page.addr + reg.addr,
                reg.reset
            ));

        } else {
            self.push_stash(1, &format!("      self.{name} = {}.{}(self, \"{name}\", {}, {:#x})\n",
                remove_rif(&self.comp_name).to_casing(Casing::Pascal),
                reg.reg_type.to_casing(Casing::Pascal),
                page.addr + reg.addr,
                reg.reset
            ));
            if let Some(desc) = self.desc_to_string(&reg.description,2) {
                self.push_stash(1, &desc);
            }
        }
    }

    fn write_rifmux_header(&mut self,  rifmux: &RifmuxInst, rif_list: &RifList, _rifmux_list: &[&RifmuxInst]) {
        self.write("from typing import final\n");
        self.write(&format!("from {} import Peripheral\n\n", self.base_module));
        for (rif_inst,_) in rif_list.iter() {
            self.write(&format!("from .{} import {}\n",
                rif_inst.name(false).to_lowercase(),
                remove_rif(&rif_inst.type_name).to_casing(Casing::Pascal)
            ));
        }
        let class_name = remove_rif(&rifmux.type_name).to_casing(Casing::Pascal);

        self.write("\n@final\n");
        self.write(&format!("class {class_name}(Peripheral):\n\n"));
        if let Some(desc) = self.desc_to_string(&rifmux.description, 1) {
            self.write(&desc);
        }
        self.write("   def __init__(self, name: str = '', addr: int = 0):\n");
        self.write("      super().__init__(name, addr)\n");
    }

    fn write_rif_inst(&mut self, rif_inst: &RifInst, cntxt: RifContext, _desc: &Description, _last_page: bool, _last_comp: bool) {
        let name = remove_rif(&rif_inst.inst_name).to_casing(Casing::Snake);
        self.write(&format!("      self.{name} = {}('{name}', {:#x} + addr)\n",
            remove_rif(&rif_inst.type_name).to_casing(Casing::Pascal),
            cntxt.addr
        ));
        if let Some(desc) = self.desc_to_string(&rif_inst.description,2) {
            self.write(&desc);
        }
    }

}