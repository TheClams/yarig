use crate::{
    comp::comp_inst::{RifFieldInst, RifInst, RifPageInst, RifRegInst, RifmuxGroupInst, RifmuxInst},
    parser::remove_rif,
    rifgen::{Description, EnumDef, EnumEntry}
};

use super::{
    casing::{Casing, ToCasing},
    gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore, InstDict, RifList},
    trait_sw::{GeneratorSw, RifContext}
};


pub struct GeneratorC {
    /// Base structure of all generators
    core: GeneratorCore,
    /// Flag when current RIF has multiple pages
    multipage : bool,
    /// Current RIF data bus width
    data_width : u8,
    /// Current RIF address bus width
    addr_width : u8,
    /// Current Component name (Rifmux or rif)
    comp_name : String,
    /// Maximum length of field name inside current register
    max_len_field_name : usize,
    /// Maximum length of field name inside current page
    max_len_reg_name : usize,
    /// Maximum length of field name inside current page
    max_len_reg_type : usize,
    /// Name for base address of each component
    base_addr_name: String,
    /// Flag overlapping register
    overlap: bool
}

impl GeneratorC {

    pub fn new(setting: GeneratorBaseSetting, base_addr_name: Option<String>) -> Self {
        GeneratorC {
            core: GeneratorCore::new(3,setting),
            data_width: 32,
            addr_width: 16,
            multipage: false,
            comp_name: "".to_owned(),
            base_addr_name : base_addr_name.unwrap_or("PERIPH_BASE_ADDR".to_owned()),
            max_len_field_name : 0,
            max_len_reg_name : 0,
            max_len_reg_type : 0,
            overlap: false
        }
    }
}

impl GeneratorBase for GeneratorC {

    const EXT : &'static str = "h";

    fn core(&self) -> &GeneratorCore {
        &self.core
    }

    fn core_mut(&mut self) -> &mut GeneratorCore {
        &mut self.core
    }

}

impl GeneratorSw for GeneratorC {
    const HAS_ENUM     : bool = true;
    const SINGLE_FILE  : bool = false;
    const HAS_UNUSED   : bool = true;
    const INST_BY_PAGE : bool = true;
    const INC_PAGENAME : bool = true;
    const INST_ARRAY   : bool = true;


    //-------- Save some state variables --------//
    /// Set the width of address/data for current RIF
    fn set_rif_info(&mut self, rif: &RifInst) {
        self.comp_name = rif.type_name.to_owned();
        self.addr_width = rif.addr_width;
        self.data_width = rif.data_width;
        self.multipage = rif.pages.len() > 1;
    }

    /// Set the max length  of field name inside a register (for pretty formatting)
    fn set_max_field_name_len(&mut self, len: usize) {
        self.max_len_field_name = len;
    }

    /// Set the max length of register name/type inside a page (for pretty formatting)
    fn set_max_reg_name_len(&mut self, name_len: usize, type_len: usize) {
        self.max_len_reg_name = name_len;
        self.max_len_reg_type = type_len;
    }

    //-------- RIF functions --------//
    // Stash organisation:
    // - 0 : Define position/reset for each field in a register
    // - 1 : Structure containing registers (one per page)
    // - 2 : Define of register Offset/reset
    /// Write RIF header start of file
    fn write_rif_header(&mut self, _rif: &RifInst, _base_addr: Option<u64>) {
        let rifname_uc = self.comp_name.to_uppercase();
        self.write(&format!("// Register definition for P_{rifname_uc}\n"));
        self.write(&format!("#ifndef __{rifname_uc}_H__\n"));
        self.write(&format!("#define __{rifname_uc}_H__\n\n"));
        self.write(         "#include <stdint.h>\n\n");
    }

    /// Write RIF end of ifdef
    fn write_rif_footer(&mut self) {
        self.pop_stash(1);
        self.write(&format!("#endif /* __{}_H__ */\n", self.comp_name.to_uppercase()));
    }

    /// Write enum start of declaration statement
    fn write_enum_header(&mut self, type_name: &str, desc: &str) {
        let etn = format!("{}_{type_name}_t", remove_rif(&self.comp_name));
        self.write(&format!("/// {}\n", desc));
        self.write(&format!("typedef enum {etn} {{\n"));
    }

    /// Write enum entry
    fn write_enum_entry(&mut self, entry: &EnumEntry , is_last: bool) {
        let sep = if is_last {""} else {","};
        self.write(&format!("    {}_{} = {}{} //!< {}\n",
            remove_rif(&self.comp_name).to_uppercase(),
            entry.name.to_uppercase(),
            entry.value,
            sep,
            entry.description.get_short()
        ));
    }

    /// Write enum end of declaration
    fn write_enum_footer(&mut self, type_name: &str) {
        self.write(&format!("}} {}_{type_name}_t;\n\n", remove_rif(&self.comp_name)));
    }

    /// Write register start of declaration statement
    fn write_reg_header(&mut self, basename: &str, reg: &RifRegInst) {
        let w = self.data_width;
        self.write(&format!("/// {} {} register bitfields\n",
            basename.to_casing(Casing::Title),
            reg.reg_type.to_casing(Casing::Title)));
        for l in reg.base_description.get().lines() {
            self.write(&format!("/// {l}\n"));
        }
        let reg_type = reg.reg_type.to_lowercase(); // self.casing(reg_type); //
        self.write(&format!("typedef union {}_{reg_type}_reg {{\n", basename.to_lowercase()));
        self.write(&format!("  uint{w}_t reg{w}; //!< Direct access to the full {reg_type} register\n", ));
        self.write("  struct {\n");
    }

    /// Write register end of declaration
    fn write_field_decl(&mut self, basename: &str, reg: &RifRegInst, field: &RifFieldInst, _enum_def: Option<&EnumDef>, _is_last: bool) {
        let field_name = self.get_field_name(reg, field);
        let name = self.casing(&field_name);
        let mask =
            if field.visibility.is_unused() {None}
            else {Some((((1_u128<<field.width)-1)<<field.lsb) as usize)};
        let desc = field.base_description.get_short(); // TODO: handle visibility/privacy
        let mask = if let Some(v) = mask {format!("0x{v:08X} ")} else {"".to_owned()};
        let l = self.max_len_field_name;
        self.write(&format!("    uint{}_t {name:<l$} : {:>2}; //!< {mask}{desc}\n",
            self.data_width, field.width,
        ));
        // Prepare some define as well for each field in a register (except for unused)
        if !field.visibility.is_unused() {
            let fieldname = name.replace('_', "").to_uppercase();
            let regname = reg.reg_type.to_uppercase();
            let name = format!("{}_{regname}_{fieldname}", basename.to_uppercase());
            self.push_stash(0, &format!("#define {name}_POS   {}\n", field.lsb));
            self.push_stash(0, &format!("#define {name}_MASK  0x{:08X}\n",(1_u128<< field.width)-1));
            self.push_stash(0, &format!("#define {name}_SMASK ({name}_MASK<<{name}_POS)\n"));
            if field.nb_frac != 0 {
                self.push_stash(0, &format!("#define {name}_FRAC    {}\n", field.nb_frac));
            }
        }
    }

    /// Write register end of declaration
    fn write_reg_footer(&mut self,  basename: &str, reg: &RifRegInst, _is_last: bool) {
        self.write("  } fields; //!< Access to bitfields\n");
        self.write(&format!("}} {}_{}_reg_t;\n\n",
            basename.to_lowercase(),
            reg.reg_type.to_lowercase()));
        self.write("\n#ifndef DOXYGEN_SHOULD_SKIP_THIS\n");
        self.pop_stash(0);
        self.write("#endif /* DOXYGEN_SHOULD_SKIP_THIS */\n\n");
    }

    /// Write register start of declaration statement
    fn write_page_header(&mut self, name: &str, page: &RifPageInst) {
        self.push_stash(1, &format!("/// {} module struct\n", name.to_casing(Casing::Title)));
        for l in page.description.get().lines() {
            self.push_stash(1, &format!("/// {l}\n"));
        }
        self.push_stash(1, &format!("typedef struct {}_regs {{\n", name.to_lowercase()));
    }

    /// Write register end of declaration
    fn write_page_footer(&mut self, name: &str, _is_last: bool) {
        self.push_stash(1, &format!("}} {}_regs_t;\n\n", name.to_lowercase()));
        self.push_stash(1, "\n#ifndef DOXYGEN_SHOULD_SKIP_THIS\n");
        self.pop_stash_to(2,1);
        self.push_stash(1, "#endif /* DOXYGEN_SHOULD_SKIP_THIS */\n\n");
    }

    /// Write the start of register instances overlap
    fn write_reginst_overlap_header(&mut self) {
        self.overlap = true;
        self.push_stash(1,"  union {\n");
    }

    /// Write the end of register instances overlap
    fn write_reginst_overlap_footer(&mut self) {
        self.overlap = false;
        self.push_stash(1,"  };\n");
    }

    /// Write unused register instances
    fn write_reginst_unused(&mut self, basename: &str, addr: u64, span: u64) {
        let mut inst_name = format!("rsvd{addr}");
        if span > 1 {
            inst_name.push_str(&format!("[{span}]"));
        }
        let type_name = format!("uint{}_t", self.data_width);
        let lt = self.max_len_reg_type + basename.len() + 1;
        let ln = self.max_len_reg_name;
        self.push_stash(1, &format!("  {type_name:<lt$} {inst_name:<ln$};\n"));
    }

    /// Write register instances
    fn write_reginst(&mut self, basename: &str, page: &RifPageInst, reg: &RifRegInst, inst_dict: &InstDict, _is_last: bool) {
        let dim = reg.array.dim();
        let lt = self.max_len_reg_type;
        let ln = self.max_len_reg_name;
        let type_name = format!("{}_reg_t", &reg.reg_type.to_lowercase());
        let desc =
            if dim > 1 {reg.base_description.get_short()}
            else {reg.description.get_short()};
        let mut inst_name = self.casing(&reg.reg_name);
        if dim > 1 {
            inst_name.push_str(&format!("[{dim}]"));
        }
        let addr = page.addr + reg.addr;
        // Increase indentation for overelapping register
        if self.overlap{
            self.push_stash(1,"  ");
        }
        self.push_stash(1,
            &format!("  {}_{type_name:<lt$} {inst_name:<ln$}; //!< 0x{addr:04X} (0x{:08X} {}): {desc}\n",
                basename.to_lowercase(),
                reg.reset,
                reg.sw_access)
        );
        // Defines register offset and reset value
        let basename_uc = basename.to_uppercase();
        let reg_name_uc = reg.reg_name.to_uppercase();
        self.push_stash(2, &format!("#define {basename_uc}_{reg_name_uc}_OFFSET {addr}\n"));
        let inst_idxs = inst_dict.get(&reg.expanded_type_name()).expect("All register instances should be in the dictionnary !");
        let mut has_multi_reset = false;
        let resets : Vec<(String,u128)> = inst_idxs.iter().map(|i| {
            let r = page.regs.get(*i as usize).unwrap();
            if r.reset != reg.reset {
                has_multi_reset = true;
            }
            (r.name().to_uppercase(), r.reset)
        }).collect();
        if has_multi_reset {
            for (k,v) in resets.iter() {
                self.push_stash(2, &format!("#define {basename_uc}_{k}_RESET {:#08X}\n", v));
            }
        } else {
            self.push_stash(2, &format!("#define {basename_uc}_{reg_name_uc}_RESET {:#08X}\n", reg.reset));
        }
    }

    //-------- RIFmux functions --------//
    /// Write RIF start of declaration statement
    fn write_rifmux_header(&mut self, rifmux: &RifmuxInst, rif_list: &RifList, _rifmux_list: &[&RifmuxInst]) {
        let name_uc = rifmux.type_name.to_uppercase();
        self.write("// Register File mapping\n");
        self.write(&format!("#ifndef __{name_uc}_H__\n"));
        self.write(&format!("#define __{name_uc}_H__\n\n"));
        // Includes
        self.write("// Includes Register File definition\n");
        for (rif,_) in rif_list.iter() {
            self.write(&format!("#include \"{}.h\"\n", rif.name(false).to_lowercase()));
        }
        self.write("\n");
    }

    /// Write RIF end of declaration
    fn write_rifmux_footer(&mut self, rifmux: &RifmuxInst) {
        self.pop_stash(0);
        self.write(&format!("\n#endif /* __{}_H__ */\n", rifmux.type_name.to_uppercase()));
    }

    /// Write RIF end of declaration
    fn write_rif_inst(&mut self, rif_inst: &RifInst, cntxt: RifContext, desc: &Description, last_page: bool, _last_comp: bool) {
        let mut base_addr_name = self.base_addr_name.clone();
        if !cntxt.group.is_empty() && cntxt.prefix.is_empty() {
            base_addr_name.push('_');
            base_addr_name.push_str(cntxt.group);
        }
        let inst_name = remove_rif(&rif_inst.inst_name);
        let mut name_tt = format!("{}{inst_name}", cntxt.prefix).to_casing(Casing::Title);
        let mut name = format!("{}{}", cntxt.prefix, &inst_name.replace('_', ""));
        let mut page_type = remove_rif(&rif_inst.type_name).to_lowercase();
        if self.multipage {
            name.push_str(&cntxt.page.replace('_', ""));
            name_tt.push(' ');
            name_tt.push_str(&cntxt.page.to_casing(Casing::Title));
            page_type.push_str(&format!("_{}",cntxt.page.to_lowercase()));
        }
        let name_uc = name.to_uppercase();
        self.write(&format!("/// {name_tt} base address: {}\n", desc.get_short()));
        self.write(&format!("#define {name_uc}_BASE_ADDR ({base_addr_name} + 0x{:08X})\n", cntxt.addr));
        self.push_stash(0, &format!("/// Pointer to {name_tt} registers\n"));
        self.push_stash(0, &format!("#define P_{name_uc} ((volatile {page_type}_regs_t* ) {name_uc}_BASE_ADDR)\n"));
        if last_page {
            self.write("\n");
        }
    }

    /// Write definition associated with a group
    fn write_rifmux_group(&mut self, group: &RifmuxGroupInst, is_last: bool) {
        self.write(&format!("/// Base address of group {}: {}\n",
            group.name.to_casing(Casing::Title),
            group.description.get_short()));
        self.write(&format!("#define {0}_{1} ({0} + 0x{2:08X})\n",
            self.base_addr_name.to_uppercase(),
            group.name.to_uppercase(),
            group.addr));
        if is_last {
            self.write("\n");
        }
    }


}
