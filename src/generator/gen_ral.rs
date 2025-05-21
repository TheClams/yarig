use crate::{
    cfg::CfgRal,
    comp::comp_inst::{RifFieldInst, RifInst, RifPageInst, RifRegInst, RifmuxInst},
    parser::remove_rif,
    rifgen::{Description, EnumDef, FieldSwKind}
};

use super::{
    gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore, InstDict, RifList},
    trait_sw::{GeneratorSw, RifContext},
};


pub struct GeneratorRal {
    /// Base structure of all generators
    core: GeneratorCore,
    /// Name of the base class for register block
    ral_class : String,
    /// Name of macro to instantiate register block
    ral_macro : Option<String>,
    /// Flag when current register is defined in another rif
    reg_is_incl : bool,
}


impl GeneratorRal {

    pub fn new(setting: GeneratorBaseSetting, extra: CfgRal) -> Self {
        let mut core = GeneratorCore::new(3,setting);
        // Override gen_inc if defined in the python settings
        if let Some(gen_inc) = extra.gen_inc {
            core.setting.gen_inc = gen_inc;
        }
        GeneratorRal {
            core,
            ral_class: extra.class.unwrap_or("uvm_reg_block".to_owned()),
            ral_macro: extra.macro_name,
            reg_is_incl: false,
        }
    }

    fn field_acc(field: &RifFieldInst) -> &str {
        match field.sw_kind {
            FieldSwKind::ReadWrite   => "\"RW\"",
            FieldSwKind::ReadOnly    => "\"RO\"",
            FieldSwKind::WriteOnly   => "\"WO\"",
            FieldSwKind::ReadClr     => "\"RC\"",
            FieldSwKind::W1Clr       => "\"W1C\"",
            FieldSwKind::W0Clr       => "\"W0C\"",
            FieldSwKind::W1Set       => "\"W1S\"",
            FieldSwKind::W1Tgl       => "\"W1T\"",
            FieldSwKind::W1Pulse(_,_)=> "\"RW\"", // Note: no good equivalent in UVM ...
            FieldSwKind::Password(_) => "\"RW\"",
        }
    }

    fn format_u128(val: u128, width: u8, is_signed: bool) -> String {
        let w = (width >> 2) as usize;
        // let width
        if width > 12 {
            let s = if is_signed {"s"} else {""};
            format!("{width}'{s}h{val:0w$X}")
        } else if is_signed && width > 1 && val >= 1<<(width-1) {
            format!("{}", val as i128 - (1<<(width)))
        } else {
            format!("{val}")
        }
    }

}

impl GeneratorBase for GeneratorRal {

    const EXT : &'static str = "sv";

    fn core(&self) -> &GeneratorCore {
        &self.core
    }

    fn core_mut(&mut self) -> &mut GeneratorCore {
        &mut self.core
    }

    fn filename_rif(&self, rif: &RifInst) -> String {
        format!("ral_{}.{}", rif.name(false) , Self::EXT)
    }

    fn filename_rifmux(&self, rif: &RifmuxInst) -> String {
        format!("ral_{}.{}", rif.type_name , Self::EXT)
    }

}

impl GeneratorSw for GeneratorRal {
    const HAS_ENUM        : bool = false;
    const SINGLE_FILE     : bool = false;
    const HAS_UNUSED      : bool = false;
    const INST_BY_PAGE    : bool = false;
    const INC_PAGENAME    : bool = false;
    const INST_ARRAY      : bool = false;
    const IS_HIERARCHICAL : bool = true;

    fn write_rif_header(&mut self, _rif: &RifInst, _base_addr: Option<u64>) {
        let name_uc = self.comp_name().to_uppercase();
        self.write(&format!("`ifndef RAL_{name_uc}\n"));
        self.write(&format!("`define RAL_{name_uc}\n"));
        self.write("\nimport uvm_pkg::*;\n\n");
    }

    fn write_rif_footer(&mut self) {
        self.write(&format!("`endif // RAL_{}\n", self.comp_name().to_uppercase()));
    }

    fn write_reg_header(&mut self, _basename: &str, reg: &RifRegInst) {
        let reg_type = reg.reg_type.to_lowercase();
        let baseclass =
            if let Some(rif) = &reg.incl {
                self.reg_is_incl = true;
                format!("ral_reg_{rif}_{reg_type}")
            }
            else {
                self.reg_is_incl = false;
                "uvm_reg".to_owned()
            };
        let comp_name = remove_rif(self.comp_name()).to_lowercase();
        self.write(&format!("class ral_reg_{comp_name}_{reg_type} extends {baseclass};\n"));
    }

    fn write_reg_footer(&mut self,  basename: &str, reg: &RifRegInst, _is_last: bool) {
        let reg_type = reg.reg_type.to_lowercase();
        self.write(&format!("\n   function new(string name = \"{basename}_{reg_type}\");\n"));
        if self.reg_is_incl {
            self.write("      super.new(name);\n");
        } else {
            self.write(&format!("      super.new(name, {}, UVM_NO_COVERAGE);\n", self.data_width()));
        }
        self.write(         "   endfunction : new\n\n");
        self.write(         "   virtual function void build();\n");
        self.pop_stash(0);
        self.write(         "   endfunction : build\n\n");
        self.write(&format!("   `uvm_object_utils(ral_reg_{basename}_{reg_type})\n\n"));
        self.write(&format!("endclass : ral_reg_{basename}_{reg_type}\n\n"));
    }

    fn write_field_decl(&mut self, _basename: &str, reg: &RifRegInst, field: &RifFieldInst, _enum_def: Option<&EnumDef>, _is_last: bool) {
        let fieldname = self.get_field_name(reg, field);
        if !self.reg_is_incl {
            let rand_s = if field.is_sw_write() {"rand "} else {""};
            self.write(&format!("   {rand_s}uvm_reg_field {fieldname};\n", ));
        }
        // Push field instantiation on stash 0
        self.push_stash(0, &format!("      this.{fieldname} = uvm_reg_field::type_id::create(\"{fieldname}\",,get_full_name());\n"));
        self.push_stash(0, &format!("      this.{fieldname}.configure(this, {}, {}, ", field.width, field.lsb));
        self.push_stash(0, &format!("{}, ", Self::field_acc(field)));
        self.push_stash(0, &format!("{}, ", if field.hw_access.is_writable() {"1"} else {"0"}));
        let rst = field.reset();
        let w = (field.width>>2) as usize;
        self.push_stash(0, &format!("{}'h{rst:0w$X}, ", field.width));
        self.push_stash(0, &format!("{}, 0, 0);\n", if field.sw_kind.is_ro() {0} else {1}));
    }

    // This is called only once on first page (INST_BY_PAGE=false)
    // All pages are merged into one to create a block of register
    fn write_page_header(&mut self, name: &str, _page: &RifPageInst) {
        self.push_stash(1, &format!("class ral_block_{name} extends {};\n", self.ral_class));
    }

    // This is called only once on last page (INST_BY_PAGE=false)
    fn write_page_footer(&mut self, name: &str, _is_last: bool) {
        self.pop_stash(1);
        self.write(&format!("\n   function new(string name = \"{name}\");\n"));
        self.write(         "      super.new(name, UVM_NO_COVERAGE);\n");
        self.write(         "   endfunction : new\n\n");
        self.write(         "   virtual function void build();\n");
        self.write(&format!("      this.default_map = create_map(\"\", 0, {}, UVM_LITTLE_ENDIAN, 0);\n", self.data_width()>>3));
        self.pop_stash(2);
        self.write(         "   endfunction : build\n\n");
        self.write(&format!("   `uvm_object_utils(ral_block_{name})\n\n"));
        self.write(&format!("endclass : ral_block_{name}\n\n"));
    }

    fn write_reginst(&mut self, _basename: &str, page: &RifPageInst, reg: &RifRegInst, inst_dict: &InstDict, _is_last: bool) {
        let regname = reg.name().to_lowercase();
        let comp_name = remove_rif(self.comp_name()).to_lowercase();
        let regtype = format!("ral_reg_{comp_name}_{}", reg.reg_type.to_lowercase());
        // Declare register instance as members of the class
        self.push_stash(1, &format!("   rand {regtype} {regname};\n"));

        // Push register instance on stash 0
        self.push_stash(2, &format!("      this.{regname} = {regtype}::type_id::create(\"{regname}\",,get_full_name());\n"));
        self.push_stash(2, &format!("      this.{regname}.configure(this, null, \"\");\n"));
        self.push_stash(2, &format!("      this.{regname}.build();\n"));
        self.push_stash(2, &format!("      this.{regname}.add_hdl_path_slice(\"{regname}__read_data\", 0, {});\n", self.data_width()));
        self.push_stash(2, &format!("      this.default_map.add_reg(this.{regname}, "));
        self.push_stash(2, &format!("`UVM_REG_ADDR_WIDTH\'h{:X}, ", page.addr + reg.addr));
        self.push_stash(2, &format!("\"{}\", 0);\n", reg.sw_access));
        let is_public = self.setting().privacy.is_public();
        let reg_1st = &inst_dict.first_inst(page, reg);
        for (fi,field) in reg.fields.iter()
                .filter(|f| !(f.visibility.is_hidden() && is_public))
                .enumerate() {
            let rand_s = if field.is_sw_write() {"rand "} else {""};
            let fieldname = self.get_field_name(reg, field);
            let idx_base = if reg_1st.is_reg_def() {field.array.idx() % field.array.dim()} else {field.array.idx()};
            let Some(base_field) = reg_1st.find_field(&field.name, idx_base) else {
                panic!("[ERROR] Field {fieldname} == {} with index {:?} (reg {:?}): Unable to find amongst {:?}",
                    field.name, field.array, reg_1st.array, reg_1st.fields.iter().map(|fi| (&fi.name, fi.array)).collect::<Vec<_>>());
            };
            let base_field_name = self.get_field_name(reg_1st, base_field);
            self.push_stash(1, &format!("   {rand_s}uvm_reg_field {regname}_{fieldname};\n"));
            self.push_stash(2, &format!("      this.{regname}_{fieldname} = this.{regname}.{base_field_name};\n"));
            let rst = field.reset();
            if rst != reg_1st.fields[fi].reset() {
                self.push_stash(2,
                    &format!("      this.{regname}_{fieldname}.set_reset({});\n",
                        Self::format_u128(rst, field.width, field.is_signed())));
            }
        }
    }

    fn write_rifmux_header(&mut self, rifmux: &RifmuxInst, rif_list: &RifList, rifmux_list: &[&RifmuxInst]) {
        let blkname = format!("ral_block_{}", &rifmux.type_name);
        let name_uc = blkname.to_uppercase();
        self.write(&format!("`ifndef {name_uc}\n"));
        self.write(&format!("`define {name_uc}\n"));
        self.write("\nimport uvm_pkg::*;\n\n");
        if self.ral_macro.is_none() {
            self.write("`ifndef ral_create_reg_block\n");
            self.write("`define ral_create_reg_block(BLOCK,  BLOCK_TYPE, OFFSET=0, PREFIX=ral_block_, PARENT=this, PATH=\"\") \\");
            self.write("   this.m_``PREFIX````BLOCK`` = ``PREFIX````BLOCK_TYPE``_rif::type_id::create(`\"``BLOCK```\", null, get_full_name()); \\");
            self.write("   this.m_``PREFIX````BLOCK``.configure(PARENT, PATH); \\");
            self.write("   this.m_``PREFIX````BLOCK``.build(); \\");
            self.write("   this.default_map.add_submap(this.m_``PREFIX````BLOCK``.default_map, OFFSET);");
            self.write("`endif\n\n");
        }
        for (rif,_) in rif_list.iter() {
            self.write(&format!("`include \"ral_{}.sv\"\n", rif.name(false).to_lowercase()));
        }
        for rifmux in rifmux_list.iter() {
            self.write(&format!("`include \"ral_{}.sv\"\n", rifmux.type_name.to_lowercase()));
        }
        self.write(&format!("\nclass {blkname} extends {};\n", self.ral_class));
    }

    fn write_rifmux_footer(&mut self, rifmux: &RifmuxInst) {
        let blkname = format!("ral_block_{}", &rifmux.type_name);
        self.write(&format!("\n   `uvm_object_utils({blkname});\n\n"));
        self.write(&format!("   function new(string name = \"{blkname}\");\n"));
        self.write(         "      super.new(name, UVM_NO_COVERAGE);\n");
        self.write(         "   endfunction : new\n\n");
        self.write(         "   virtual function void build();\n");
        self.write(         "      this.default_map = create_map(\"\", 0, 4, UVM_LITTLE_ENDIAN, 0);\n");
        self.pop_stash(0);
        self.write(         "   endfunction : build\n\n");
        self.write(&format!("endclass : {blkname}\n\n"));
        self.write(&format!("`endif // {}\n", blkname.to_uppercase()));
    }

    fn write_rif_inst(&mut self, rif_inst: &RifInst, cntxt: RifContext, _desc: &Description, _last_page: bool, _last_comp: bool) {
        let instname = remove_rif(&rif_inst.inst_name);
        let typename = remove_rif(&rif_inst.type_name);
        let macroname = self.ral_macro.clone().unwrap_or("ral_create_reg_block".to_owned());
        self.write(&format!("   ral_block_{typename} m_ral_block_{instname};\n"));
        self.push_stash(0,&format!("      `{macroname}({instname}, {typename}, 'h{:08x})\n", cntxt.addr));
    }

    fn write_rifmux_inst(&mut self, rifmux_inst: &RifmuxInst, addr: u64, _last_comp: bool) {
        let instname = remove_rif(&rifmux_inst.inst_name);
        let typename = remove_rif(&rifmux_inst.type_name);
        let macroname = self.ral_macro.clone().unwrap_or("ral_create_reg_block".to_owned());
        self.write(&format!("   ral_block_{typename} m_ral_block_{instname};\n"));
        self.push_stash(0,&format!("      `{macroname}({instname}, {typename}, 'h{addr:08x})\n"));
    }

}