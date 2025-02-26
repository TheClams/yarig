use std::fs::create_dir_all;

use crate::{
    comp::comp_inst::{Comp, RifFieldInst, RifInst, RifRegInst, RifmuxInst},
    parser::remove_rif, rifgen::{Access, Description, EnumDef, EnumEntry}
};

use super::{
    casing::{Casing, ToCasing},
    gen_common::{GeneratorBase, InstDict, RifList}
};

/// Current context for a component instance (Rif or rifmux)
pub struct RifContext<'a> {
    pub prefix: &'a str,
    pub group: &'a str,
    pub page: &'a str,
    pub addr: u64,
}

impl RifContext<'_> {
    pub fn new<'a>(prefix: &'a str, group: &'a str, page: &'a str, addr: u64) -> RifContext<'a> {
        RifContext {prefix, group, page, addr}
    }
}

/// Trait to implement generator for software control (C, python, ...)
/// 
#[allow(dead_code)]
pub trait GeneratorSw : GeneratorBase {

    /// Declare enum type
    const HAS_ENUM : bool = false;
    /// Generate a single file for the rifmux and all its RIF definition
    const SINGLE_FILE : bool = false;
    /// Unused part of the register also have field declaration
    const HAS_UNUSED : bool = false;
    /// Create one structure per page instead of grouping all register and ignoring the page structure
    const INST_BY_PAGE : bool = false;
    /// Include pagename inside basename of registers
    const INC_PAGENAME : bool = false;
    /// Create only one register instance per array (false to create instance per element)
    const INST_ARRAY : bool = false;
    /// Instantiate rifmux instead of flatenning all rifs instance
    const IS_HIERARCHICAL : bool = false;
    /// Create register type declaration before register instance
    const HAS_REG_DECL : bool = true;

    /// Main generator function
    fn gen_all(&mut self, obj: &Comp) -> Result<(), Box<dyn std::error::Error>> {
        // Create output directory if it does not exist
        create_dir_all(self.setting().path.clone())?;
        // Create resource file if needed
        self.create_resource()?;
        // Dispatch generator: RIF or rifmux
        match obj {
            Comp::Rifmux(rifmux) => {
                let riflist = RifList::new(rifmux, !Self::IS_HIERARCHICAL);
                if !Self::SINGLE_FILE && !self.setting().gen_inc.is_empty() {
                    let gen_all = self.setting().gen_inc.first()==Some(&"*".to_owned());
                    for rif in riflist.iter() {
                        if !gen_all && !self.setting().is_gen_inc(rif) {
                            continue;
                        }
                        self.gen_rif(rif, false)?;
                    }
                }
                self.gen_rifmux(rifmux, &riflist)
            }
            Comp::Rif(rif) => self.gen_rif(rif, true),
            // Nothing todo for external RIF
            Comp::External(_) => Ok(())
        }
    }

    /// Create base resource related to the generator
    fn create_resource(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Generate structure associated to a RIF
    fn gen_rif(&mut self, rif: &RifInst, is_top: bool) -> Result<(), Box<dyn std::error::Error>> {
        self.set_rif_info(rif);
        self.write_rif_header(rif, is_top);
        let basename = remove_rif(&rif.type_name);
        let nb_byte = (rif.data_width >> 3) as u64;
        let is_public = self.setting().privacy.is_public();
        // Declare types for enum
        if Self::HAS_ENUM {
            for def in rif.enum_defs.iter().filter(|d| !d.name.starts_with("doc:")) {
                let etn = match def.name.rfind("::") {
                    Some(pos) => &def.name[pos+2..],
                    None => &def.name,
                };
                let etn = etn.strip_prefix("e_").unwrap_or(etn);
                self.write_enum_header(etn, &def.description);
                let mut enum_entries = def.iter().peekable();
                while let Some(enum_entry) = enum_entries.next() {
                    self.write_enum_entry(enum_entry, enum_entries.peek().is_none());
                }
                self.write_enum_footer(etn);
            }
        }
        // Collect instances to easily get the one used in declaration
        let inst_dict = InstDict::new(&rif.pages, is_public);
        // Declare one struct per register type
        for (idx, page) in rif.pages.iter().filter(|p| !p.is_external()).enumerate() {
            let pname =
                if Self::INC_PAGENAME && rif.pages.len() > 1 {format!("{}_{}", basename,page.name)}
                else {basename.to_owned()};
            let is_last_page = idx==rif.pages.len()-1;
            // Parse all register instance to get length, for pretty formatting
            let len_name = page.regs.iter().map(|r| r.reg_name.len()).max().expect("Page should have registers");
            let len_type = 6+page.regs.iter().map(|r| r.reg_type.len()).max().expect("Page should have registers");
            self.set_max_reg_name_len(len_name, len_type);
            //
            if Self::HAS_REG_DECL {
                let mut regs = page.iter_reg_type().filter(|r| r.sw_access!=Access::NA).peekable();
                while let Some(reg) = regs.next()  {
                    let max_len = reg.fields.iter()
                        .map(|f| self.get_field_name(reg,f).len())
                        .max().expect("Registers should have fields");
                    self.set_max_field_name_len(max_len);
                    self.write_reg_header(&pname, reg);
                    self.write_fields_decl(&rif, &pname, reg);
                    self.write_reg_footer(&pname, reg, regs.peek().is_none());
                }
            }
            // Instantiate all registers
            // Call page header only once on first page if instance are not grouped by page
            if idx==0 || Self::INST_BY_PAGE {
                self.write_page_header(&pname, &page.description);
            }
            let mut overlap = false;
            let mut addr = 0;
            // TODO: handle visibility ?
            let mut regs = page.regs.iter()
                .filter(|r| (r.array.idx()==0 || !Self::INST_ARRAY) && !r.sw_access.is_na())
                .peekable();
            while let Some(reg) = regs.next() {
                // println!("Register instance: {} ({}) @ {} | {:?}", reg.reg_name, reg.reg_type, reg.addr, reg.array );
                // Detect end of overlap
                if overlap && reg.addr >= addr {
                    self.write_reginst_overlap_footer();
                    overlap = false;
                }
                // Detect start of overlap
                if !overlap {
                    if let Some(reg_next) = regs.peek() {
                        if reg.addr == reg_next.addr && !overlap {
                            self.write_reginst_overlap_header();
                            overlap = true;
                        }
                    }
                }
                // Detect non-contiguous register: TODO add field overlap to register
                if reg.addr > addr {
                    self.write_reginst_unused(&pname, addr, (reg.addr - addr) / nb_byte);
                }
                let reg_1st = inst_dict.first_inst(page, reg);
                let is_last_reg = is_last_page && regs.peek().is_none();
                self.write_reginst(&pname, page.addr, reg, reg_1st, is_last_reg);
                if !Self::HAS_REG_DECL {
                    self.write_fields_decl(&rif, &pname, reg);
                    self.write_reg_footer(&pname, reg, regs.peek().is_none());
                }

                // Calculate expected next address
                let nb = reg.array.dim().max(1);
                addr = reg.addr + nb_byte * nb as u64;

            }
            if overlap {
                self.write_reginst_overlap_footer();
            }
            if is_last_page || Self::INST_BY_PAGE {
                self.write_page_footer(&pname, is_last_page);
            }
        }
        // End the RIF declaration
        self.write_rif_footer();
        // Write file if top or one file per component
        if !Self::SINGLE_FILE || is_top {
            self.save(&self.filename_rif(rif))?;
        }
        Ok(())
    }

    /// Save information for the current RIF
    fn set_rif_info(&mut self, rif: &RifInst);

    /// Set the max length  of field name inside a register (for pretty formatting)
    fn set_max_field_name_len(&mut self, _len: usize) {}

    /// Set the max length of register name/type inside a page (for pretty formatting)
    fn set_max_reg_name_len(&mut self, _name_len: usize, _type_len: usize) {}

    /// Write RIF start of declaration statement
    fn write_rif_header(&mut self, rif: &RifInst, is_top: bool);

    /// Write RIF end of declaration
    fn write_rif_footer(&mut self);

    /// Write enum start of declaration statement
    fn write_enum_header(&mut self, _type_name: &str, _desc: &str) {}

    /// Write enum entry
    fn write_enum_entry(&mut self, _entry: &EnumEntry , _is_last: bool) {}

    /// Write enum end of declaration
    fn write_enum_footer(&mut self, _type_name: &str) {}

    /// Write register start of declaration statement
    fn write_reg_header(&mut self, basename: &str, reg: &RifRegInst);

    /// Write register end of declaration
    fn write_reg_footer(&mut self,  basename: &str, reg: &RifRegInst, is_last: bool);

    fn write_fields_decl(&mut self, rif: &RifInst, basename: &str, reg: &RifRegInst) {
        let is_public = self.setting().privacy.is_public();
        let mut fields = reg.fields.iter().filter(|f| !(f.visibility.is_hidden() && is_public)).peekable();
        let mut pos_l = 0;
        while let Some(f) = fields.next() {
            if pos_l != f.lsb {
                self.write_unused_field_decl(basename, reg, pos_l, f.lsb - pos_l, false);
            }
            let enum_def =
                if let Some(enum_name) = f.enum_kind.name() {
                    rif.get_enum_def(enum_name).ok()
                } else {
                    None
                };
            self.write_field_decl(basename, reg, f, enum_def, fields.peek().is_none());
            pos_l = f.lsb + f.width;
        }
        // Fill remaining bits if any
        if pos_l < rif.data_width {
            self.write_unused_field_decl(basename, reg, pos_l, rif.data_width - pos_l, true);
        }
    }

    /// Write register end of declaration
    fn write_field_decl(&mut self, basename: &str, reg: &RifRegInst, field: &RifFieldInst, enum_defs: Option<&EnumDef>, is_last: bool);

    fn get_field_name(&self, reg: &RifRegInst, f: &RifFieldInst) -> String {
        if f.is_reserved() && self.setting().privacy.is_public() {
            format!("rsvd{}",f.lsb)
        } else if f.array.dim() > 1 || reg.array.dim()==0 || reg.array.is_inst() {
            f.name_flat()
        } else {
            f.name.to_owned()
        }
    }

    fn get_field_iter<'a>(&self, reg: &'a RifRegInst, ) -> impl std::iter::Iterator<Item = &'a RifFieldInst> {
        reg.fields.iter()
            .filter(|f| {
                !(f.visibility.is_hidden() && self.setting().privacy.is_public())
            })
            .peekable()
    }

    /// Write register end of declaration
    fn write_unused_field_decl(&mut self, basename: &str, reg: &RifRegInst, lsb: u8, width: u8, is_last: bool) {
        if Self::HAS_UNUSED {
            let field = RifFieldInst::new_unused(lsb, width);
            self.write_field_decl(basename, reg, &field, None, is_last);
        }
    }

    /// Write register start of declaration statement
    fn write_page_header(&mut self, name: &str, desc: &Description);

    /// Write register end of declaration
    fn write_page_footer(&mut self, name: &str, is_last: bool);

    /// Write the start of register instances overlap
    fn write_reginst_overlap_header(&mut self) {}

    /// Write the end of register instances overlap
    fn write_reginst_overlap_footer(&mut self) {}

    /// Write register instances
    fn write_reginst(&mut self, basename: &str, base_addr: u64, reg: &RifRegInst, reg_1st: &RifRegInst, is_last: bool);

    /// Write unused register instances
    /// Default to nothing
    fn write_reginst_unused(&mut self, _basename: &str, _addr: u64, _span: u64) {}

    /// Generate structure associated to a RIFmux
    fn gen_rifmux(&mut self, rifmux: &RifmuxInst, rif_list: &RifList) -> Result<(), Box<dyn std::error::Error>> {
        let rifmux_list : Vec<&RifmuxInst> = rifmux.components.iter()
            .filter_map(|c| if let Comp::Rifmux(m) = &c.inst {Some(m)} else {None})
            .collect();
        self.write_rifmux_header(rifmux, rif_list, &rifmux_list);
        self.scan_rifmux(rifmux, "", 0, true )?;
        self.write_rifmux_footer(rifmux);
        self.save(&self.filename_rifmux(rifmux))
    }

    /// Scan rifmux components
    fn scan_rifmux(&mut self, rifmux: &RifmuxInst, top_name: &str, offset: u64, last_scan : bool) -> Result<(), Box<dyn std::error::Error>> {
        let prefix = if top_name.is_empty() {
            "".to_owned()
        } else {
            format!("{}_",top_name)
        };
        let mut comps = rifmux.components.iter().filter(|c| !c.is_external()).peekable();
        while let Some(comp) = comps.next()  {
            let addr = offset + comp.addr;
            let last_comp = comps.peek().is_none() && last_scan;
            match &comp.inst {
                Comp::Rifmux(r) => {
                    if Self::IS_HIERARCHICAL {
                        self.write_rifmux_inst(r, addr, last_comp)
                    } else {
                        let comp_name = format!("{prefix}{}",r.inst_name.to_casing(Casing::Pascal));
                        self.scan_rifmux(r, &comp_name, addr, last_comp)?;
                    }
                }
                Comp::Rif(r) => {
                    self.set_rif_info(r);
                    let mut pages = r.pages.iter().peekable();
                    while let Some(page) = pages.next() {
                        let desc = if page.description.is_empty() {&r.description} else {&page.description};
                        let cntxt = RifContext::new(&prefix, &comp.group, &page.name, page.addr + addr);
                        self.write_rif_inst(r, cntxt, desc, pages.peek().is_none(), last_comp);
                        if !Self::INST_BY_PAGE {
                            break;
                        }
                    }
                    if Self::SINGLE_FILE {
                        self.gen_rif(r,false)?;
                    }
                }
                Comp::External(_) => {},
            }
        }
        Ok(())
    }

    /// Write RIF start of declaration statement
    fn write_rifmux_header(&mut self, rifmux: &RifmuxInst, rif_list: &RifList, rifmux_list: &[&RifmuxInst]);

    /// Write RIF end of declaration
    fn write_rifmux_footer(&mut self, rifmux: &RifmuxInst);

    /// Write RIF instance
    fn write_rif_inst(&mut self, rif_inst: &RifInst, cntxt: RifContext, desc: &Description, last_page: bool, last_comp: bool);

    /// Write RIF mux instance
    fn write_rifmux_inst(&mut self, _rifmux_inst: &RifmuxInst, _addr: u64, _last_comp: bool) {}

}
