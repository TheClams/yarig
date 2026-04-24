use std::ops::Deref;

use crate::{
    comp::comp_inst::{Comp, RifFieldInst, RifInst, RifPageInst, RifRegInst, RifmuxGroupInst, RifmuxInst},
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
    pub group_addr: u64,
}

impl RifContext<'_> {
    pub fn new<'a>(prefix: &'a str, group: &'a str, page: &'a str, addr: u64, group_addr: u64) -> RifContext<'a> {
        RifContext {prefix, group, page, addr, group_addr}
    }
}

/// Trait to implement generator for software control (C, python, ...)
/// 
#[allow(unused_variables)]
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
    /// True when target support field array. False to split field array into individual field for each element of the array
    const FIELD_ARRAY : bool = false;
    /// Instantiate rifmux instead of flatenning all rifs instance
    const IS_HIERARCHICAL : bool = false;
    /// Create register type declaration before register instance
    const HAS_REG_DECL : bool = true;
    /// Create register type declaration for each variant of interrupts
    const INTR_VARIANT_DECL : bool = false;

    /// Main generator function
    fn gen_all(&mut self, obj: &Comp) -> Result<(), Box<dyn std::error::Error>> {
        // Create resource file if needed
        self.create_resource()?;
        // Dispatch generator: RIF or rifmux
        match obj {
            Comp::Rifmux(rifmux) => {
                let riflist = RifList::new(rifmux, !Self::IS_HIERARCHICAL);
                if !Self::SINGLE_FILE && !self.setting().gen_inc.is_empty() {
                    let gen_all = self.setting().is_gen_all();
                    for (rif, info) in riflist.iter() {
                        if !gen_all && !self.setting().is_gen_inc(rif) {
                            continue;
                        }
                        self.set_comp((*rif).into(), true);
                        self.gen_rif(rif, info.first().map(|e| e.0))?;
                    }
                }
                self.gen_rifmux(rifmux, &riflist)
            }
            Comp::Rif(rif) => {
                self.set_comp(rif.deref().into(), false);
                self.gen_rif(rif, None)
            }
            // Nothing todo for external RIF
            Comp::External(_) => Ok(())
        }
    }

    /// Create base resource related to the generator
    fn create_resource(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Generate structure associated to a RIF
    fn gen_rif(&mut self, rif: &RifInst, base_addr: Option<u64>) -> Result<(), Box<dyn std::error::Error>> {
        self.set_rif_info(rif);
        self.write_rif_header(rif, base_addr);
        let rif_name = remove_rif(&rif.type_name);
        let nb_byte = (rif.data_width >> 3) as u64;
        let is_public = self.is_public();
        // Declare types for enum
        if Self::HAS_ENUM {
            for def in rif.enum_defs.iter().filter(|d| !d.name.starts_with("doc:")) {
                let etn = match def.name.rfind("::") {
                    Some(pos) => &def.name[pos+2..],
                    None => &def.name,
                };
                let etn = etn.strip_prefix("e_").unwrap_or(etn);
                self.write_enum_header(etn, def);
                let mut enum_entries = def.iter().peekable();
                while let Some(enum_entry) = enum_entries.next() {
                    self.write_enum_entry(enum_entry, enum_entries.peek().is_none());
                }
                self.write_enum_footer(etn, def);
            }
        }
        // Collect instances to easily get the one used in declaration
        let inst_dict = InstDict::new(&rif.pages, is_public);
        // Declare one struct per register type
        for (idx, page) in rif.pages.iter().filter(|p| !p.is_external()).enumerate() {
            let prefix =
                if rif.pages.len() > 1 {format!("{}_{}", rif_name, page.name.replace('_', "").to_lowercase())}
                else {rif_name.to_owned()};
            let basename = if Self::INC_PAGENAME {&prefix} else {rif_name};
            let page_name = if Self::INST_BY_PAGE {&prefix} else {rif_name};
            let is_last_page = idx==rif.pages.len()-1;
            // Parse all register instance to get length, for pretty formatting
            let len_name = page.regs.iter().map(|r| self.casing(&r.reg_name).len()).max().expect("Page should have registers");
            let len_type = 6+page.regs.iter().map(|r| self.casing(&r.reg_type).len()).max().expect("Page should have registers");
            self.set_max_reg_name_len(len_name, len_type);
            //
            if Self::HAS_REG_DECL {
                let mut regs = page.iter_reg_type().filter(|r| r.sw_access!=Access::NA).peekable();
                while let Some(reg) = regs.next()  {
                    // Prepare an optional iterator on interupts instance
                    let mut intr_regs = if Self::INTR_VARIANT_DECL && reg.is_intr() {
                        Some(page.inst_by_type(&reg.reg_type).skip(1).peekable())
                    } else {
                        None
                    };
                    let last_type = regs.peek().is_none();
                    let last_reg = intr_regs.as_mut().is_none_or(|x| x.peek().is_none());
                    // Get max lenegth of field name to allow alignment
                    let max_len = reg.fields.iter()
                        .map(|f| self.get_field_name(reg,f).len())
                        .max().expect("Registers should have fields");
                    self.set_max_field_name_len(max_len);
                    //
                    self.write_reg_header(basename, reg);
                    // When register is defined as an array
                    let nb_regs_def = reg.array.dim_def() as usize;
                    let mut regs_rst : Vec<u128> = Vec::with_capacity(nb_regs_def);
                    regs_rst.push(reg.reset);
                    if nb_regs_def > 0 {
                        regs_rst.extend(page.inst_by_type(&reg.reg_type).skip(1).map(|r| r.reset));
                    }
                    self.write_fields_decl(rif, basename, reg, &regs_rst);
                    self.write_reg_footer(basename, reg, last_type && last_reg);
                    //
                    if let Some(mut intr_regs) = intr_regs {
                        while let Some(r) = intr_regs.next() {
                            self.write_reg_header(basename, r);
                            self.write_fields_decl(rif, basename, r, &regs_rst);
                            self.write_reg_footer(basename, r, last_type && intr_regs.peek().is_none());
                        }
                    }
                }
            }
            // Instantiate all registers
            // Call page header only once on first page if instance are not grouped by page
            if idx==0 || Self::INST_BY_PAGE {
                self.write_page_header(page_name, page);
            }
            let mut overlap = false;
            let mut addr = 0;
            let mut regs = page.regs.iter()
                .filter(|r|
                    (r.array.idx()==0 || !Self::INST_ARRAY) &&
                    !(r.sw_access.is_na() || (is_public && r.visibility.is_hidden())))
                .peekable();
            while let Some(reg) = regs.next() {
                // println!("Register instance: {} ({}) @ {} | {:?}", reg.reg_name, reg.reg_type, reg.addr, reg.array );
                // Detect end of overlap
                if overlap && reg.addr >= addr {
                    self.write_reginst_overlap_footer();
                    overlap = false;
                }
                // Detect start of overlap
                if !overlap && let Some(reg_next) = regs.peek() && reg.addr == reg_next.addr {
                    self.write_reginst_overlap_header();
                    overlap = true;
                }
                // Detect non-contiguous register: TODO add field overlap to register
                if reg.addr > addr {
                    self.write_reginst_unused(basename, addr, (reg.addr - addr) / nb_byte);
                }
                let is_last_reg = is_last_page && regs.peek().is_none();
                self.write_reginst(basename, page, reg, &inst_dict, is_last_reg);
                if !Self::HAS_REG_DECL {
                    self.write_fields_decl(rif, basename, reg, &[]);
                    self.write_reg_footer(basename, reg, is_last_reg);
                }

                // Calculate expected next address
                let nb = reg.array.dim().max(1);
                addr = reg.addr + nb_byte * nb as u64;

            }
            if overlap {
                self.write_reginst_overlap_footer();
            }
            if is_last_page || Self::INST_BY_PAGE {
                self.write_page_footer(page_name, is_last_page);
            }
        }
        // End the RIF declaration
        self.write_rif_footer();
        // Write file if top or one file per component
        let is_top = base_addr.is_none();
        if !Self::SINGLE_FILE || is_top {
            let fname = if is_top && self.setting().fname.is_some() {
                self.setting().fname.clone().unwrap()
            } else {
                self.filename_rif(rif)
            };
            self.save(&fname, is_top)?;
        }
        Ok(())
    }

    /// Save information for the current RIF
    fn set_rif_info(&mut self, rif: &RifInst) {}

    /// Set the max length  of field name inside a register (for pretty formatting)
    fn set_max_field_name_len(&mut self, len: usize) {}

    /// Set the max length of register name/type inside a page (for pretty formatting)
    fn set_max_reg_name_len(&mut self, name_len: usize, type_len: usize) {}

    /// Write RIF start of declaration statement
    fn write_rif_header(&mut self, rif: &RifInst, base_addr: Option<u64>) {}

    /// Write RIF end of declaration
    fn write_rif_footer(&mut self) {}

    /// Write enum start of declaration statement
    fn write_enum_header(&mut self, type_name: &str, def: &EnumDef) {}

    /// Write enum entry
    fn write_enum_entry(&mut self, entry: &EnumEntry , is_last: bool) {}

    /// Write enum end of declaration
    fn write_enum_footer(&mut self, type_name: &str, def: &EnumDef) {}

    /// Write register start of declaration statement
    fn write_reg_header(&mut self, basename: &str, reg: &RifRegInst) {}

    /// Write register end of declaration
    fn write_reg_footer(&mut self,  basename: &str, reg: &RifRegInst, is_last: bool) {}

    /// Write fields declaration
    fn write_fields_decl(&mut self, rif: &RifInst, basename: &str, reg: &RifRegInst, regs_rst: &[u128]) {
        let is_public = self.is_public();
        let mut fields = reg.fields.iter()
            .filter(|f| !(f.visibility.is_hidden() && is_public) && (!Self::FIELD_ARRAY || f.array.idx()==0))
            .peekable();
        let mut pos_l = 0;
        // let fields_c = fields.clone();
        // println!("[WriteFieldsDecl] Reg {} ({}): Fields {:?}", reg.reg_name, Self::FIELD_ARRAY, fields_c.map(|f| format!("{} idx={}", f.name(), f.array.idx())).collect::<Vec<_>>() );
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
            self.write_field_decl(basename, reg, regs_rst, f, enum_def, fields.peek().is_none());
            pos_l = f.lsb + f.width;
        }
        // Fill remaining bits if any
        if pos_l < rif.data_width {
            self.write_unused_field_decl(basename, reg, pos_l, rif.data_width - pos_l, true);
        }
    }

    /// Write register end of declaration
    fn write_field_decl(&mut self, basename: &str, reg: &RifRegInst, regs_rst: &[u128], field: &RifFieldInst, enum_defs: Option<&EnumDef>, is_last: bool) {}

    /// Return a field name with proper casing
    fn get_field_name(&self, reg: &RifRegInst, field: &RifFieldInst) -> String {
        self.core().get_field_name(reg, field, Self::FIELD_ARRAY)
    }

    /// Return an iterator on fields of a register, skipping any private field when visibiility is set to public
    fn get_field_iter<'a>(&self, reg: &'a RifRegInst, ) -> impl std::iter::Iterator<Item = &'a RifFieldInst> {
        reg.fields.iter()
            .filter(|f| {
                !(f.visibility.is_hidden() && self.is_public())
            })
    }

    /// Write register end of declaration
    fn write_unused_field_decl(&mut self, basename: &str, reg: &RifRegInst, lsb: u8, width: u8, is_last: bool) {
        if Self::HAS_UNUSED {
            let field = RifFieldInst::new_unused(lsb, width);
            self.write_field_decl(basename, reg, &[], &field, None, is_last);
        }
    }

    /// Write register start of declaration statement
    fn write_page_header(&mut self, name: &str, page: &RifPageInst) {}

    /// Write register end of declaration
    fn write_page_footer(&mut self, name: &str, is_last: bool) {}

    /// Write the start of register instances overlap
    fn write_reginst_overlap_header(&mut self) {}

    /// Write the end of register instances overlap
    fn write_reginst_overlap_footer(&mut self) {}

    /// Write register instances
    fn write_reginst(&mut self, basename: &str, page: &RifPageInst, reg: &RifRegInst, inst_dict: &InstDict, is_last: bool) {}

    /// Write unused register instances
    /// Default to nothing
    fn write_reginst_unused(&mut self, basename: &str, addr: u64, span: u64) {}

    /// Generate structure associated to a RIFmux
    fn gen_rifmux(&mut self, rifmux: &RifmuxInst, rif_list: &RifList) -> Result<(), Box<dyn std::error::Error>> {
        self.set_comp(rifmux.into(), true);
        let rifmux_list : Vec<&RifmuxInst> = rifmux.components.iter()
            .filter_map(|c| if let Comp::Rifmux(m) = &c.inst {Some(m.deref())} else {None})
            .collect();
        self.write_rifmux_header(rifmux, rif_list, &rifmux_list);
        let mut groups = rifmux.groups.iter().peekable();
        while let Some(group) = groups.next() {
            self.write_rifmux_group(group, groups.peek().is_none());
        }
        self.scan_rifmux(rifmux, "", 0, true )?;
        self.set_comp(rifmux.into(), true);
        self.write_rifmux_footer(rifmux);
        // Save file
        let fname = self.setting().fname.clone()
            .unwrap_or(self.filename_rifmux(rifmux));
        self.save(&fname, true)
    }

    /// Scan rifmux components
    fn scan_rifmux(&mut self, rifmux: &RifmuxInst, top_name: &str, offset: u64, last_scan : bool) -> Result<(), Box<dyn std::error::Error>> {
        let prefix = if top_name.is_empty() {
            "".to_owned()
        } else {
            format!("{top_name}_")
        };
        let mut comps = rifmux.components.iter().filter(|c| !c.is_external()).peekable();
        while let Some(comp) = comps.next()  {
            let addr = offset + comp.full_addr(&rifmux.groups);
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
                    self.set_comp(r.deref().into(), true);
                    self.set_rif_info(r);
                    let mut pages = r.pages.iter().peekable();
                    let group_addr = rifmux.groups.iter().find(|g| g.name==comp.group).map(|g| g.addr).unwrap_or(0);
                    while let Some(page) = pages.next() {
                        let desc = if page.description.is_empty(self.is_public()) {&r.description} else {&page.description};
                        let cntxt = RifContext::new(&prefix, &comp.group, &page.name, page.addr + addr, group_addr);
                        self.write_rif_inst(r, cntxt, desc, pages.peek().is_none(), last_comp);
                        if !Self::INST_BY_PAGE {
                            break;
                        }
                    }
                    if Self::SINGLE_FILE {
                        self.gen_rif(r,Some(addr))?;
                    }
                }
                Comp::External(_) => {},
            }
        }
        Ok(())
    }

    /// Write RIF start of declaration statement
    fn write_rifmux_header(&mut self, rifmux: &RifmuxInst, rif_list: &RifList, rifmux_list: &[&RifmuxInst]) {}

    /// Write RIF end of declaration
    fn write_rifmux_footer(&mut self, rifmux: &RifmuxInst) {}

    /// Write definition associated with a group
    fn write_rifmux_group(&mut self, group: &RifmuxGroupInst, is_last: bool) {}

    /// Write RIF instance
    fn write_rif_inst(&mut self, rif_inst: &RifInst, cntxt: RifContext, desc: &Description, last_page: bool, last_comp: bool) {}

    /// Write RIF mux instance
    fn write_rifmux_inst(&mut self, rifmux_inst: &RifmuxInst, addr: u64, last_comp: bool) {}

}
