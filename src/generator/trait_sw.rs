use std::fs::create_dir_all;

use crate::{
    comp::comp_inst::{Comp, RifFieldInst, RifInst, RifRegInst, RifmuxInst},
    parser::remove_rif, rifgen::{Access, Description, EnumEntry}
};

use super::{
    casing::{Casing, ToCasing},
    gen_common::{GeneratorBaseSetting, GeneratorCore, RifList}
};

/// Trait to implement generator for software control (C, python, ...)
/// 
#[allow(dead_code)]
pub trait GeneratorSw {

    /// File extension
    const EXT : &'static str;
    /// Declare enum type
    const HAS_ENUM : bool = false;
    /// Generate a single file for the rifmux and all its RIF definition
    const SINGLE_FILE : bool = false;
    /// Unused part of the register also have field declaration
    const HAS_UNUSED : bool = false;
    /// Create one structure per page instead of grouping all register and ignoring the page structure
    const INST_BY_PAGE : bool = false;

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

    /// Main generator function
    fn gen(&mut self, obj: &Comp) -> Result<(), Box<dyn std::error::Error>> {
        // Create output directory if it does not exist
        create_dir_all(self.setting().path.clone())?;
        match obj {
            Comp::Rifmux(rifmux) => {
                let riflist = RifList::new(rifmux);
                if !Self::SINGLE_FILE && !self.setting().gen_inc.is_empty() {
                    let gen_all = self.setting().gen_inc.first()==Some(&"*".to_owned());
                    for rif in riflist.iter() {
                        if !gen_all && !self.setting().is_gen_inc(rif) {
                            continue;
                        }
                        self.gen_rif(rif, false)?;
                    }
                }
                self.gen_rifmux(rifmux, &riflist, true)
            }
            Comp::Rif(rif) => self.gen_rif(rif, true),
            // Nothing todo for external RIF
            Comp::External(_) => Ok(())
        }
    }

    /// Generate structure associated to a RIF
    fn gen_rif(&mut self, rif: &RifInst, is_top: bool) -> Result<(), Box<dyn std::error::Error>> {
        self.set_rif_info(&rif.type_name, rif.addr_width, rif.data_width, rif.pages.len());
        self.write_rif_header(is_top);
        let basename = remove_rif(&rif.type_name);
        let nb_byte = (rif.data_width >> 3) as u64;
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
        // Declare one struct per register type
        let mut pages = rif.pages.iter().filter(|p| !p.is_external()).peekable();
        while let Some(page) = pages.next() {
            let pname =
                if rif.pages.len() > 1 {format!("{}_{}", basename,page.name)}
                else {basename.to_owned()};
            for reg in page.iter_reg_type().filter(|r| r.sw_access!=Access::NA) {
                let max_len = reg.fields.iter()
                    .map(|f| self.get_field_name(reg,f).len())
                    .max().expect("Registers should have fields");
                self.set_max_field_name_len(max_len);
                self.write_reg_header(&pname, &reg.reg_type, reg.base_description.get());
                let mut pos_l = 0;
                for f in reg.fields.iter() {
                    if pos_l != f.lsb {
                        self.write_unused_field_decl(&pname, reg, pos_l, f.lsb - pos_l);
                    }
                    self.write_field_decl(&pname, reg, f);
                    pos_l = f.lsb + f.width;
                }
                // Fill remaining bits if any
                if pos_l < rif.data_width {
                    self.write_unused_field_decl(&pname, reg, pos_l, rif.data_width - pos_l);
                }
                // End of register declaration
                self.write_reg_footer(&pname, &reg.reg_type);
            }
            // Instantiate all registers
            let len_name = page.regs.iter().map(|r| r.reg_name.len()).max().expect("Page should have registers");
            let len_type = 6+page.regs.iter().map(|r| r.reg_type.len()).max().expect("Page should have registers");
            self.set_max_reg_name_len(len_name, len_type);
            self.write_page_header(&pname, &page.description);
            let mut overlap = false;
            let mut addr = 0;
            // TODO: handle visibility ?
            let mut regs = page.regs.iter().filter(|r| r.array.idx()==0).peekable();
            while let Some(reg) = regs.next() {
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
                self.write_reginst(&pname, page.addr, reg);
                    // &reg.reg_type, &reg.reg_name, reg.array.dim(),
                    // reg.sw_access, reg.reset, desc);

                // Calculate expected next address
                let nb = reg.array.dim().max(1);
                addr = reg.addr + nb_byte * nb as u64;

            }
            if overlap {
                self.write_reginst_overlap_footer();
            }
            self.write_page_footer(&pname, pages.peek().is_none());
        }
        // End the RIF declaration
        self.write_rif_footer();
        // Write file if top or one file per component
        if !Self::SINGLE_FILE || is_top {
            self.save(&format!("{}.{}", rif.name(false), Self::EXT))?;
        }
        Ok(())
    }

    /// Set the width of address/data for current RIF
    fn set_rif_info(&mut self, name: &str, addr_w: u8, data_w: u8, nb_page: usize);

    /// Set the max length  of field name inside a register (for pretty formatting)
    fn set_max_field_name_len(&mut self, _len: usize) {}

    /// Set the max length of register name/type inside a page (for pretty formatting)
    fn set_max_reg_name_len(&mut self, _name_len: usize, _type_len: usize) {}

    /// Write RIF start of declaration statement
    fn write_rif_header(&mut self, is_top: bool);

    /// Write RIF end of declaration
    fn write_rif_footer(&mut self);

    /// Write enum start of declaration statement
    fn write_enum_header(&mut self, type_name: &str, desc: &str);

    /// Write enum entry
    fn write_enum_entry(&mut self, entry: &EnumEntry , is_last: bool);

    /// Write enum end of declaration
    fn write_enum_footer(&mut self, type_name: &str);

    /// Write register start of declaration statement
    fn write_reg_header(&mut self, basename: &str, reg_type: &str, desc: &str);

    /// Write register end of declaration
    fn write_reg_footer(&mut self,  basename: &str, reg_type: &str);

    /// Write register end of declaration
    fn write_field_decl(&mut self, pname: &str, r: &RifRegInst, field: &RifFieldInst);

    fn get_field_name(&self, r: &RifRegInst, f: &RifFieldInst) -> String {
        if f.is_reserved() && self.setting().privacy.is_public() {
            format!("rsvd{}",f.lsb)
        } else if f.array.dim() > 1 || r.array.dim()==0 || r.array.is_inst() {
            f.name_flat()
        } else {
            f.name.to_owned()
        }
    }


    /// Write register end of declaration
    fn write_unused_field_decl(&mut self, pname: &str, reg: &RifRegInst, lsb: u8, width: u8) {
        if Self::HAS_UNUSED {
            let field = RifFieldInst::new_unused(lsb, width);
            self.write_field_decl(pname, reg, &field);
        }
    }

    /// Write register start of declaration statement
    fn write_page_header(&mut self, name: &str, desc: &Description);

    /// Write register end of declaration
    fn write_page_footer(&mut self, name: &str, is_last: bool);

    /// Write the start of register instances overlap
    fn write_reginst_overlap_header(&mut self);

    /// Write the end of register instances overlap
    fn write_reginst_overlap_footer(&mut self);

    /// Write register instances
    fn write_reginst(&mut self, basename: &str, base_addr: u64, reg: &RifRegInst);

    /// Write unused register instances
    /// Default to nothing
    fn write_reginst_unused(&mut self, _basename: &str, _addr: u64, _span: u64) {}

    /// Generate structure associated to a RIFmux
    fn gen_rifmux(&mut self, rifmux: &RifmuxInst, rif_list: &RifList, _is_top: bool) -> Result<(), Box<dyn std::error::Error>> {
        self.write_rifmux_header(&rifmux.inst_name, rif_list);
        self.scan_rifmux(rifmux, "", 0)?;
        self.write_rifmux_footer(&rifmux.inst_name);
        self.save(&format!("{}.{}", &rifmux.inst_name, Self::EXT))
    }

    /// Scan rifmux components
    fn scan_rifmux(&mut self, rifmux: &RifmuxInst, top_name: &str, offset: u64) -> Result<(), Box<dyn std::error::Error>> {
        let prefix = if top_name.is_empty() {
            "".to_owned()
        } else {
            format!("{}_",top_name)
        };
        for comp in rifmux.components.iter() {
            match &comp.inst {
                Comp::Rifmux(r) => {
                    let comp_name = format!("{prefix}{}",r.inst_name.to_casing(Casing::Pascal));
                    self.scan_rifmux(r, &comp_name, offset + comp.addr)?;
                }
                Comp::Rif(r) => {
                    self.set_rif_info(&r.type_name, r.addr_width, r.data_width, r.pages.len());
                    let mut pages = r.pages.iter().peekable();
                    while let Some(page) = pages.next() {
                        let desc = if page.description.is_empty() {&r.description} else {&page.description};
                        let addr = page.addr + comp.addr + offset;
                        self.write_rifinst_ref(&prefix, &comp.group, r, &page.name, addr, desc, pages.peek().is_none());
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
    fn write_rifmux_header(&mut self, name: &str, rif_list: &RifList);

    /// Write RIF end of declaration
    fn write_rifmux_footer(&mut self, name: &str);

    /// Write RIF end of declaration
    fn write_rifinst_ref(&mut self, prefix: &str, group: &str, rif_inst: &RifInst, page_name: &str, addr: u64, desc: &Description, is_last: bool);

}
