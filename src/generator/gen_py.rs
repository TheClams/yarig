use std::collections::{BTreeMap, BTreeSet};

use crate::{
    comp::comp_inst::{RifFieldInst, RifInst, RifRegInst, RifmuxInst},
    parser::remove_rif,
    rifgen::Description};

use super::{
    casing::{Casing, ToCasing},
    gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore, RifList},
    trait_sw::{GeneratorSw, RifContext}
};


pub struct GeneratorPy {
    /// Base structure of all generators
    core: GeneratorCore,
    /// Base python module defining Peripheral, Register and Field classes
    base_module : String,
    /// Current Component name (Rifmux or rif)
    comp_name : String,
    /// Current RIF data bus width
    data_width : u8,
    /// Dictionary containing the register owning a field array definition
    field_parent : BTreeMap<String,String>,
    /// Dictionary containing the list of field array in a register
    field_array : BTreeSet<String>,
}


impl GeneratorPy {
    const DEFAULT_BASECLASS : &'static str = include_str!("resources/regmap.py");

    pub fn new(setting: GeneratorBaseSetting, base_module: Option<String>) -> Self {
        GeneratorPy {
            core: GeneratorCore::new(2,setting),
            comp_name: "".to_owned(),
            data_width: 32,
            base_module: base_module.unwrap_or(".regmap".to_owned()),
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
    }

    /// Create the regmap.py containing base class if not defined in another python module
    fn create_resource(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.base_module==".regmap" {
            let path : std::path::PathBuf = [
                self.setting().path.clone(),
                "regmap.py".into()
            ].iter().collect();
            std::fs::write(path, Self::DEFAULT_BASECLASS.as_bytes())?;
        }
        Ok(())
    }

    fn write_rif_header(&mut self, rif: &RifInst, _is_top: bool) {
        let rif_name = remove_rif(&self.comp_name).to_casing(Casing::Pascal);
        self.write("from typing import final\n");
        self.write(&format!("from {} import Field, Register, Peripheral\n\n", self.base_module));
        self.write("@final\n");
        self.write(&format!("class {rif_name}(Peripheral):\n"));
        if let Some(desc) = self.desc_to_string(&rif.description,1) {
            self.write(&desc);
        }
        // Cleanup field parent dictionnary
        self.field_parent.clear();
    }

    fn write_rif_footer(&mut self) {}

    fn write_reg_header(&mut self, _basename: &str, reg: &RifRegInst) {
        let typename = reg.reg_type.to_casing(Casing::Pascal);
        self.write(       "\n   @final\n");
        self.write(&format!("   class {typename}(Register):\n"));
        if let Some(desc) = self.desc_to_string(&reg.base_description,2) {
            self.write(&desc);
        }
        // Cleanup field array dictionnary
        self.field_array.clear();
    }

    // End of register declaration: add the init function with all the field instance
    fn write_reg_footer(&mut self, _basename: &str, reg: &RifRegInst) {
        self.write("\n      def __init__(self, parent: None | Peripheral, name: str, addr: int, init: None|int = None) :\n");
        let ro = if reg.sw_access.is_writable() {"False"} else {"True"};
        self.write(&format!("         super().__init__(parent, name, addr, {ro}, init)\n"));
        self.pop_stash(0);
    }

    fn write_field_decl(&mut self, _basename: &str, reg: &RifRegInst, field: &RifFieldInst) {
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
        self.write(&format!("\n      class {field_type}(Field):\n"));
        //
        self.write(&format!("         pos : int = {}\n", field.lsb));
        self.write(&format!("         width : int = {}\n", field.width));
        self.write(&format!("         value : int = {}\n", field.reset()));
        self.write(&format!("         signed : bool = {}\n",
            if field.is_signed() {"True"} else {"False"}));
        //
        let mut flags = Vec::new();
        if field.sw_kind.is_special() {
            flags.push(field.sw_kind.access_str());
        }
        if reg.is_intr() {
            flags.push("interrupt");
        }
        if !flags.is_empty() {
            self.write(&format!("         flags : list[str] = {:?}\n", flags));
        }
    }

    fn write_page_header(&mut self, _name: &str, _desc: &Description) {}

    fn write_page_footer(&mut self, _name: &str, _is_last: bool) {
        self.write("\n\n   def __init__(self, addr: int):\n");
        self.write("      super().__init__(addr)\n");
        self.pop_stash(1)
    }

    fn write_reginst(&mut self, _basename: &str, base_addr: u64, reg: &RifRegInst, _reg_1st: &RifRegInst) {
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
                base_addr + reg.addr,
                reg.reset
            ));

        } else {
            self.push_stash(1, &format!("      self.{name} = {}.{}(self, \"{name}\", {}, {:#x})\n",
                remove_rif(&self.comp_name).to_casing(Casing::Pascal),
                reg.reg_type.to_casing(Casing::Pascal),
                base_addr + reg.addr,
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
        for rif_inst in rif_list.iter() {
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
        self.write("   def __init__(self, addr: int = 0):\n");
        self.write("      super().__init__(addr)\n");
   }

    fn write_rifmux_footer(&mut self, _rifmux: &RifmuxInst) {}

    fn write_rif_inst(&mut self, rif_inst: &RifInst, cntxt: RifContext, _desc: &Description, _is_last: bool) {
        self.write(&format!("      self.{} = {}({:#x})\n",
            remove_rif(&rif_inst.inst_name).to_casing(Casing::Snake),
            remove_rif(&rif_inst.type_name).to_casing(Casing::Pascal),
            cntxt.addr
        ));
        if let Some(desc) = self.desc_to_string(&rif_inst.description,2) {
            self.write(&desc);
        }
    }

}