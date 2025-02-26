use crate::{
    comp::comp_inst::{RifFieldInst, RifInst, RifRegInst, RifmuxInst},
    parser::remove_rif,
    rifgen::{Description, EnumDef}
};

use super::{
    gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore, RifList},
    trait_sw::{GeneratorSw, RifContext}
};


pub struct GeneratorJson(GeneratorCore);

impl GeneratorJson {

    pub fn new(setting: GeneratorBaseSetting) -> Self {
        GeneratorJson(GeneratorCore::new(1,setting))
    }

    fn desc_to_string(&mut self, desc: &Description) -> String {
        desc.get().replace("\n","\\n")
    }

}

impl GeneratorBase for GeneratorJson {

    const EXT : &'static str = "json";

    fn core(&self) -> &GeneratorCore {
        &self.0
    }

    fn core_mut(&mut self) -> &mut GeneratorCore {
        &mut self.0
    }
}


impl GeneratorSw for GeneratorJson {
    const HAS_ENUM        : bool = true;
    const SINGLE_FILE     : bool = false;
    const HAS_UNUSED      : bool = false;
    const INST_BY_PAGE    : bool = false;
    const INC_PAGENAME    : bool = false;
    const INST_ARRAY      : bool = false;
    const IS_HIERARCHICAL : bool = false;
    const HAS_REG_DECL    : bool = false;

    fn set_rif_info(&mut self, _rif: &RifInst) {}

    fn write_rif_header(&mut self, _rif: &RifInst, _is_top: bool) {
        self.write("{\n");
    }

    fn write_rif_footer(&mut self) {
        self.write("}");
    }

    fn write_reg_header(&mut self, _basename: &str, _reg: &RifRegInst) {}

    fn write_reginst(&mut self, _basename: &str, base_addr: u64, reg: &RifRegInst, _reg_1st: &RifRegInst, _is_last: bool) {
        let reg_name = self.casing(&reg.name());
        let desc = self.desc_to_string(&reg.description);
        let ro = if reg.sw_access.is_writable() {"false"} else {"true"};
        // Generate a flag array
        let mut flags : Vec<&str> = Vec::new();
        if reg.is_external() {
            flags.push("external");
        }
        if reg.is_intr() {
            flags.push("interrupt");
        }
        self.write(&format!("   \"{reg_name}\" : {{\n"));
        self.write(&format!("      \"addr\" : {},\n", base_addr + reg.addr));
        self.write(&format!("      \"desc\" : \"{desc}\",\n"));
        self.write(&format!("      \"readonly\" : {ro},\n"));
        self.write(&format!("      \"flags\" : {flags:?},\n"));
        self.write(         "      \"fields\" : {\n");
    }

    fn write_field_decl(&mut self, _basename: &str, _reg: &RifRegInst, field: &RifFieldInst, enum_def: Option<&EnumDef>, is_last: bool) {
        let tab = " ".repeat(3*3);
        let name = self.casing(&field.name());
        let signed = if field.is_signed() {"true"} else {"false"};
        let desc = self.desc_to_string(&field.description);
        let sw_kind = field.sw_kind.access_str().to_lowercase();
        self.write(&format!("{tab}\"{name}\" : {{\n"));
        self.write(&format!("{tab}   \"pos\" : {},\n", field.lsb));
        self.write(&format!("{tab}   \"width\" : {},\n", field.width));
        self.write(&format!("{tab}   \"value\" : {},\n", field.reset()));
        self.write(&format!("{tab}   \"signed\" : {signed},\n"));
        self.write(&format!("{tab}   \"kind\" : \"{sw_kind}\",\n"));
        if let Some(enum_def) = enum_def {
            self.write(&format!("{tab}   \"enum\" : [\n"));
            let mut entries = enum_def.values.iter().peekable();
            while let Some(entry) = entries.next() {
                let sep = if entries.peek().is_none() {""} else {","};
                self.write(&format!("{tab}      {{\n"));
                self.write(&format!("{tab}         \"name\" : \"{}\",\n", entry.name));
                self.write(&format!("{tab}         \"value\" : {},\n", entry.value));
                let enum_desc = self.desc_to_string(&entry.description);
                self.write(&format!("{tab}         \"desc\" : \"{enum_desc}\"\n"));
                self.write(&format!("{tab}      }}{sep}\n"));

            }
            self.write(&format!("{tab}   ],\n"));
        }
        self.write(&format!("{tab}   \"desc\" : \"{desc}\"\n"));
        let sep = if is_last {""} else {","};
        self.write( &format!("{tab}}}{sep}\n"));
    }

    fn write_reg_footer(&mut self, _basename: &str, _reg: &RifRegInst, is_last: bool) {
        // Close the fields entry
        self.write(         "      }\n");
        // Close the register entry
        let sep = if is_last {""} else {","};
        self.write(&format!("   }}{sep}\n"));
    }

    fn write_page_header(&mut self, _name: &str, _desc: &Description) {}

    fn write_page_footer(&mut self, _name: &str, _is_last: bool) {}

    fn write_rifmux_header(&mut self, _rifmux: &RifmuxInst, _rif_list: &RifList, _rifmux_list: &[&RifmuxInst]) {
        self.write("{\n");
    }

    fn write_rifmux_footer(&mut self, _rifmux: &RifmuxInst) {
        self.write("}");
    }

    fn write_rif_inst(&mut self, rif_inst: &RifInst, cntxt: RifContext, desc: &Description, _last_page: bool, last_comp: bool) {
        let desc = self.desc_to_string(desc);
        let inst_name = self.casing(&remove_rif(&rif_inst.inst_name));
        let type_name = self.casing(&remove_rif(&rif_inst.type_name));
        self.write(&format!("   \"{inst_name}\" : {{\n"));
        self.write(&format!("      \"type\" : {type_name},\n"));
        self.write(&format!("      \"group\" : {},\n", cntxt.group));
        self.write(&format!("      \"addr\" : {},\n", cntxt.addr));
        self.write(&format!("      \"desc\" : {desc},\n"));
        let sep = if last_comp {""} else {","};
        self.write("      }\n");
        self.write(&format!("   }}{sep}\n"));
    }

}