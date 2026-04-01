use std::ops::Deref;

use crate::{
    comp::comp_inst::{val_str, Comp, CompInst, RifInst, RifRegInst, RifmuxGroupInst},
    parser::remove_rif,
    rifgen::{EnumDef, FieldSwKind, ResetVal}
};

use super::gen_common::{GeneratorBase, InstDict, RifInstInfo, RifList, Skippable};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TableKind {
    Rifmux, RifInst, Page, RegInst, Layout, Field, FieldRsvd
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CellKind {
    Addr, Offset, RifType, RegType, Inst, Reset, Desc, Field, Bits, Access
}

impl CellKind {
    pub fn label(&self) -> &str {
        match self {
            CellKind::Addr    => "Address",
            CellKind::Offset  => "Offset",
            CellKind::RifType => "Type",
            CellKind::RegType => "Type",
            CellKind::Inst    => "Name",
            CellKind::Reset   => "Reset",
            CellKind::Desc    => "Description",
            CellKind::Field   => "Field",
            CellKind::Bits    => "Bits",
            CellKind::Access  => "Access",
        }
    }

    pub fn is_type(&self) -> bool {
        matches!(self, CellKind::RifType | CellKind::RegType)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LinkKind {
    Top, Page, Field
}

impl std::fmt::Display for LinkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkKind::Top   => write!(f, "Top"),
            LinkKind::Page  => write!(f, "Page summary"),
            LinkKind::Field => write!(f, "Fields details"),
        }
    }
}

/// Convert latex equation enclosed in backtick (`) into MathML
pub fn desc_ml(txt: &str) -> String {
    let mut desc = String::with_capacity(txt.len());
    let mut is_eq = false;
    for t in txt.split('`') {
        // println!("Desc part = {t} ({is_eq})");
        if is_eq {
            if let Ok(t_ml) = latex2mathml::latex_to_mathml(t, latex2mathml::DisplayStyle::Inline) {
                desc.push_str(&t_ml);
            } else {
                desc.push_str(t);
            }
        } else {
            desc.push_str(t);
        }
        is_eq = !is_eq;
    }
    desc
}


/// Trait to implement generator for documentation
/// A RIFMux starts with a table containing all the RIFs instance, followed by one paragraph per RIF type
/// The document format for a RIF is the following:
/// - a first section with a table summary of all register instance
/// - one section by register type with two table:
///   * first table list all register instance with their reset value and description.
///     This is not shown when compact mode is enabled and there is only one instance
///   * Second table showing the register layout (field position),
///     followed by a description of all fields
#[allow(unused_variables)]
pub trait GeneratorDoc : GeneratorBase {

    /// Display register layout table
    const HAS_LAYOUT : bool = false;
    /// Show register reset in top summary
    const SHOW_RESET : bool = true;
    /// Show type name summary tables (top/register)
    const SHOW_TYPE : bool = false;
    /// Show register instance even when there is only one instance
    const SHOW_SINGLE_REG : bool = false;
    /// Show unused part of a register
    const SHOW_UNUSED : bool = false;

    /// Main generator function
    fn gen_all(&mut self, obj: &Comp) -> Result<(), Box<dyn std::error::Error>> {
        //
        self.write_header(remove_rif(obj.get_name()));
        let filename;
        match obj {
            Comp::Rifmux(rifmux) => {
                self.set_comp(rifmux.deref().into(), true);
                filename = self.filename_rifmux(rifmux);
                let name = remove_rif(&rifmux.inst_name);
                let desc = rifmux.description.get_split(self.is_public());
                if !self.setting().skip.contains(&Skippable::RifmuxTitle) {
                    self.write_rif_title((name,0), &desc.0);
                }
                if let Some(desc_detail) = desc.1 {
                    let desc_detail = self.sanitize(&desc_detail);
                    self.write_info(&desc_detail);
                }
                // Table with all RIF instances
                let w = ((rifmux.addr_width+3) >> 2) as usize;
                self.write_table_title(TableKind::Rifmux, "Summary", "topSummary");
                self.write_table_row_top_header(TableKind::Rifmux);
                for k in [CellKind::Addr, CellKind::RifType, CellKind::Inst, CellKind::Desc] {
                    if !Self::SHOW_TYPE  && k==CellKind::RifType {continue;}
                    self.write_table_cell_top((TableKind::Rifmux, k), 0, k.label(), "");
                }
                self.write_table_row_top_footer();
                for c in rifmux.components.iter() {
                    self.add_rifmux_entry(c, w,  0, None, &rifmux.groups);
                }
                self.write_table_footer(TableKind::Rifmux);
                self.set_comp(rifmux.deref().into(), true);
                // Split output -> save top level now
                if self.setting().split {
                    self.write_footer(obj.get_name());
                    let fname = self.setting().fname.clone().unwrap_or(filename.clone());
                    self.save(&fname, true)?;
                }
                // Add description of all rif types
                let rif_list = RifList::new(rifmux, true);
                for (i,(rif,info)) in rif_list.iter().enumerate() {
                    self.set_comp((*rif).into(), true);
                    if self.setting().split {
                        let name = rif.name(false);
                        let basename = remove_rif(&name);
                        self.write_header(basename);
                        self.add_rif(rif, 1, &[])?;
                        self.write_footer(basename);
                        self.save(&format!("{name}.{}", Self::EXT), false)?;
                    } else {
                        self.add_rif(rif, i+1, info)?;
                    }
                }
            }
            Comp::Rif(rif) => {
                filename = self.filename_rif(rif);
                self.set_comp(rif.deref().into(), false);
                self.add_rif(rif, 1, &[])?;
            },
            // Nothing todo for external RIF
            Comp::External(_) => return Ok(()),
        }
        if !self.setting().split || obj.is_rif() {
            self.write_footer(obj.get_name());
            // Write file
            let fname = self.setting().fname.clone().unwrap_or(filename);
            self.save(&fname, true)?;
        }
        Ok(())
    }

    /// Add rifmux row in a table composed of 4 column:
    /// Address, Type, Instance and short description
    fn add_rifmux_entry(
        &mut self,
        comp: &CompInst,
        w: usize,
        offset: u64,
        top_name: Option<&str>,
        groups: &[RifmuxGroupInst],
    ) {
        let rif_name = self.casing(remove_rif(comp.inst.get_name()));
        let instname = if let Some(top) = top_name {format!("{top}.{rif_name}")} else {rif_name.to_owned()};
        let addr = comp.full_addr(groups) + offset;
        match &comp.inst {
            Comp::Rifmux(c) => {
                for sub in c.components.iter() {
                    self.add_rifmux_entry(sub, w, addr, Some(&instname), &c.groups);
                }
            },
            inst => {
                let tn = remove_rif(inst.get_type());
                if let Comp::Rif(rif) = inst {
                    self.set_comp(rif.deref().into(), true);
                    self.set_rif_info(rif);
                }
                self.write_rifmux_entry(&instname, tn, (addr,w), &inst.get_desc_short(self.is_public()), Self::SHOW_TYPE);
            }
        }
    }

    fn write_rifmux_entry(&mut self, inst_name: &str, type_name: &str, addr: (u64, usize), desc: &str, show_type: bool) {
        self.write_table_row_header(None);
        let w = addr.1;
        self.write_table_cell((TableKind::Rifmux, CellKind::Addr   ), 0, &format!("0x{:0w$X}", addr.0), "", None);
        if show_type {
            self.write_table_cell((TableKind::Rifmux, CellKind::RifType), 0, type_name, type_name, None);
        }
        self.write_table_cell((TableKind::Rifmux, CellKind::Inst   ), 0, inst_name, type_name, None);
        self.write_table_cell((TableKind::Rifmux, CellKind::Desc   ), 0, &self.sanitize(desc), "", None);
        self.write_table_row_footer();
    }

    /// Add RIF description
    fn add_rif(&mut self, rif: &RifInst, idx: usize, info: &[RifInstInfo]) -> Result<(), String> {
        let rif_name = remove_rif(&rif.type_name);
        let desc = rif.base_description.get_split(self.is_public());
        self.set_rif_info(rif);
        if !self.setting().skip.contains(&Skippable::RifTitle) {
            self.write_rif_title((rif_name, idx), &desc.0);
        }
        if let Some(desc_detail) = desc.1 {
            let desc_detail = self.sanitize(&desc_detail);
            self.write_info(&desc_detail);
        }
        self.add_reg_summary(rif, info);
        if !info.is_empty() {
            self.add_link(LinkKind::Top, "topSummary");
        }
        let base_addr = if info.len() == 1 {Some(info[0].0)} else {None};
        self.add_reg_detail(rif, idx, base_addr)
    }

    /// Add register summary
    fn add_reg_summary(&mut self, rif: &RifInst, info: &[RifInstInfo]) {
        let rif_name = remove_rif(&rif.type_name);
        let addr_w = ((rif.addr_width+3)>>2) as usize;
        let data_w = ((rif.data_width+3)>>2) as usize;
        let is_public = self.core().setting.privacy.is_public();
        // Extract a base address if there is only one
        let offset = if info.len() == 1 {info[0].0} else {0};
        let addr_col = if info.len() > 1 {CellKind::Offset} else {CellKind::Addr};
        if info.len() > 1 {
            self.add_rif_summary(rif, info);
        }
        self.write_reg_summary_header(rif);
        for page in rif.pages.iter() {
            // TODO: check hidden
            let mut id_page = format!("regmap.{rif_name}");
            if rif.pages.len() > 1 {
                id_page.push('.');
                id_page.push_str(&page.name);
            }
            self.write_table_title(TableKind::Page, &page.name, &id_page);
            self.write_table_row_top_header(TableKind::Page);
            for k in [addr_col, CellKind::RifType, CellKind::Inst, CellKind::Reset, CellKind::Desc] {
                if !Self::SHOW_RESET && k==CellKind::Reset {continue;}
                if !Self::SHOW_TYPE  && k==CellKind::RifType {continue;}
                self.write_table_cell_top((TableKind::Rifmux, k), 0, k.label(), "");
            }
            self.write_table_row_top_footer();
            for reg in page.regs.iter().filter(|r| !(is_public && r.visibility.is_hidden())) {
                let reg_type = self.casing(&reg.expanded_type_name());
                let reg_name = self.casing(&reg.name_i());
                let addr = offset + page.addr + reg.addr;
                let id_reg = &format!("{rif_name}.{reg_type}");
                let desc = self.sanitize(&reg.get_desc_short(self.is_public()));
                self.write_table_row_header(None);
                self.write_table_cell((TableKind::Page, CellKind::Addr), 0, &format!("0x{addr:0addr_w$X}"), "", None);
                if Self::SHOW_TYPE {
                    self.write_table_cell((TableKind::Page, CellKind::RegType), 0, &reg_type, id_reg, None);
                }
                self.write_table_cell((TableKind::Page, CellKind::Inst), 0, &reg_name, id_reg, None);
                if Self::SHOW_RESET {
                    self.write_table_cell((TableKind::Page, CellKind::Reset), 0, &format!("0x{:0data_w$X}", reg.reset), "", None);
                }
                self.write_table_cell((TableKind::Page, CellKind::Desc), 0, &desc, "", None);
                self.write_table_row_footer();
            }
            self.write_table_footer(TableKind::Page);
        }
    }

    fn write_reg_summary_header(&mut self, rif: &RifInst) {}

    /// Summary of RIF instances when there is more than one
    fn add_rif_summary(&mut self, rif: &RifInst, infos: &[RifInstInfo]) {
        let w = ((rif.addr_width+3) >> 2) as usize;
        self.write_table_title(TableKind::RifInst, "RIF instances", "rifInstSummary");
        self.write_table_row_top_header(TableKind::RifInst);
        for k in [CellKind::Addr, CellKind::Inst, CellKind::Desc] {
            self.write_table_cell_top((TableKind::RifInst, k), 0, k.label(), "");
        }
        self.write_table_row_top_footer();
        let type_name = remove_rif(&rif.type_name);
        for info in infos.iter() {
            self.write_rifmux_entry(&info.1, type_name, (info.0,w), &info.2, false);
        }
        self.write_table_footer(TableKind::RifInst);
    }

    /// Add register details: table with register instance followed by table with fields description
    fn add_reg_detail(&mut self, rif: &RifInst, idx_c: usize, base_addr: Option<u64>)  -> Result<(),String> {
        let rif_name = remove_rif(&rif.type_name);
        let addr_w = ((rif.addr_width+3)>>2) as usize;
        let data_w = ((rif.data_width+3)>>2) as usize;
        let is_public = self.core().setting.privacy.is_public();
        let addr_col = if base_addr.is_none() {CellKind::Offset} else {CellKind::Addr};
        let offset = base_addr.unwrap_or(0);
        let reg_headers = [addr_col, CellKind::Inst, CellKind::Reset, CellKind::Desc];
        let inst_dict = InstDict::new(&rif.pages, is_public);
        self.write_reg_detail_header(rif);
        for (idx_p, page) in rif.pages.iter().enumerate() {
            let page_name = &page.name;
            let mut id_page = format!("regmap.{rif_name}");
            if rif.pages.len() > 1 {
                id_page.push('.');
                id_page.push_str(&page.name);
            }
            // Add paragraph per page only if more than one
            let desc = page.description.get_split(self.is_public());
            self.write_page_title((rif_name, idx_c), (page_name, idx_p+1), desc);
            let mut idx_r = 0;
            let regs = page.regs.iter()
                .enumerate()
                .filter(|(_,r)| !(is_public && r.visibility.is_hidden()));
            for (idx_ri,reg) in regs {
                // Check not already defined
                let reg_type = reg.expanded_type_name();
                let Some(instances) = inst_dict.get(&reg_type) else {
                    return Err(format!("Unable to find register type {rif_name}.{reg_type} in instance dict: {:?}", inst_dict.keys().collect::<Vec<&String>>()))
                };
                if instances.first() != Some(&(idx_ri as u16)) {
                    continue;
                }
                let id_reg = &format!("{rif_name}.{reg_type}");
                let reg_impl = rif.get_hw_reg(&reg.group_type);
                // Title
                idx_r += 1;
                let desc = reg.base_description.get_split(self.is_public());
                let reg_type = self.casing(&reg_type);
                // let addr = if let Some(ba) = base_addr {Some(ba+reg.addr)} else {None};
                let addr = base_addr.map(|ba| ba+reg.addr);
                self.write_reg_title((rif_name, idx_c), (page_name, idx_p+1), (&reg_type, idx_r), &self.sanitize(&desc.0), addr);
                if let Some(desc_detail) = desc.1 {
                    self.write_info(&self.sanitize(&desc_detail));
                }
                // Table with all register instance for current type
                if Self::SHOW_SINGLE_REG || instances.len() > 1 {
                    // Register instance summary
                    self.write_table_title(TableKind::RegInst, &format!("{reg_type} instances"), &format!("reginsts.{id_reg}"));
                    let hdr = if instances.len() > 1 {&reg_headers} else {&reg_headers[..3]};
                    self.write_table_row_top_header(TableKind::RegInst);
                    for k in hdr {
                         self.write_table_cell_top((TableKind::RegInst, *k), 0, k.label(), "");
                    }
                    self.write_table_row_top_footer();
                    for inst_idx in instances.iter() {
                        let Some(inst) = page.regs.get(*inst_idx as usize) else {
                            return Err(format!("Instance index {} out of range (max {}) !!!", inst_idx, page.regs.len()));
                        };
                        let reg_name = self.casing(&inst.name_i());
                        let id_name = format!("inst.{rif_name}.{reg_name}");
                        let addr = offset + page.addr + inst.addr;
                        self.write_table_row_header(Some(&id_name));
                        self.write_table_cell((TableKind::RegInst, CellKind::Addr ), 0, &format!("0x{addr:0addr_w$X}"), "", None);
                        self.write_table_cell((TableKind::RegInst, CellKind::Inst ), 0, &reg_name, &reg_type, None);
                        self.write_table_cell((TableKind::RegInst, CellKind::Reset), 0, &format!("0x{:0data_w$X}", inst.reset), "", None);
                        if instances.len() > 1 {
                            let desc = self.sanitize(&inst.get_desc_short(self.is_public()));
                            self.write_table_cell((TableKind::RegInst, CellKind::Desc), 0, &desc, "", None);
                        }
                        self.write_table_row_footer();
                    }
                    self.write_table_footer(TableKind::RegInst);
                }
                // Register Layout
                if Self::HAS_LAYOUT {
                    self.write_table_title(TableKind::Layout, &format!("{id_reg} Layout"), &format!("layout.{id_reg}"));
                    // Bit number
                    self.write_table_row_header(None);
                    self.write_table_cell((TableKind::Layout, CellKind::Desc), 0, "Bit", "", None);
                    for i in (0..rif.data_width).rev() {
                        self.write_table_cell((TableKind::Layout, CellKind::Desc), 1, &format!("{i}"), "", None);
                    }
                    self.write_table_row_footer();
                    // Field name
                    self.write_table_row_header(None);
                    self.write_table_cell((TableKind::Layout, CellKind::Desc), 0, "Field", "", None);
                    let mut last_pos = rif.data_width;
                    for f in reg.fields.iter().rev().filter(|f| !(f.visibility.is_hidden() && is_public)) {
                        let fieldname = self.core().get_field_name(reg, f, false);
                        // Insert reserved in unoccupied bits
                        if f.msb()+1 < last_pos {
                            let w = last_pos - (f.msb()+1);
                            self.write_table_cell((TableKind::Layout, CellKind::Field), w.into(), "", "", None);
                        }
                        self.write_table_cell((TableKind::Layout, CellKind::Field), f.width.into(), &fieldname, "", None);
                        last_pos = f.lsb;
                    }
                    if last_pos!=0 {
                        self.write_table_cell((TableKind::Layout, CellKind::Field), last_pos.into(), "", "", None);
                    }
                    self.write_table_row_footer();
                    // Reset value
                    self.write_table_row_header(None);
                    self.write_table_cell((TableKind::Layout, CellKind::Desc), 0, "Reset", "", None);
                    for i in (0..rif.data_width).rev() {
                        self.write_table_cell((TableKind::Layout, CellKind::Desc), 1, &format!("{}", (reg.reset >> i)&1), "", None);
                    }
                    self.write_table_row_footer();
                    self.write_table_footer(TableKind::Layout);
                }
                let reg_insts : Vec<&RifRegInst> =  instances.iter().filter_map(|idx| page.regs.get(*idx as usize)).collect();
                // Fields Details
                // No details for derived interrupt register except if there is no layout summarizing the fields
                let is_intr_derived = reg.intr_info.0.is_derived();
                if !is_intr_derived || !Self::HAS_LAYOUT {
                    self.write_table_title(TableKind::Field, &reg_type, &format!("fields.{id_reg}") );
                    self.write_table_row_top_header(TableKind::Field);
                    for k in [CellKind::Bits, CellKind::Inst, CellKind::Access, CellKind::Reset, CellKind::Desc] {
                        self.write_table_cell_top((TableKind::Field, k), 0, k.label(), "");
                    }
                    self.write_table_row_top_footer();
                    let mut last_pos = rif.data_width;
                    for f in reg.fields.iter().rev().filter(|f| !(f.visibility.is_hidden() && is_public)) {
                        // Insert reserved in unoccupied bits
                        if Self::SHOW_UNUSED && f.msb()+1 < last_pos {
                            let w = last_pos - (f.msb()+1);
                            let pos = if w==1 {format!("{}",last_pos-1)} else {format!("{}:{}", last_pos-1, f.msb()+1)};
                            self.write_table_row_header(None);
                            self.write_table_cell((TableKind::FieldRsvd, CellKind::Bits), 0, &pos, "", None);
                            self.write_table_cell((TableKind::FieldRsvd, CellKind::Inst), 0, "", "", None);
                            self.write_table_cell((TableKind::FieldRsvd, CellKind::Access), 0, "RO", "", None);
                            self.write_table_cell((TableKind::FieldRsvd, CellKind::Reset), 0, "0", "", None);
                            self.write_table_cell((TableKind::FieldRsvd, CellKind::Desc), 0, "", "", None);
                            self.write_table_row_footer();
                        }

                        let fieldname = self.core().get_field_name(reg, f, false);
                        self.write_table_row_header(Some(&format!("{rif_name}.{reg_type}.{fieldname}")));
                        let pos = if f.width==1 {format!("{}",f.lsb)} else {format!("{}:{}",f.msb(), f.lsb)};
                        self.write_table_cell((TableKind::Field, CellKind::Bits), 0, &pos, "", None);
                        self.write_table_cell((TableKind::Field, CellKind::Inst), 0, &fieldname, "", None);
                        // let access = if is_intr_derived {&FieldSwKind::ReadWrite} else {&f.sw_kind};
                        self.write_table_cell((TableKind::Field, CellKind::Access), 0, Self::access_str(&f.sw_kind), "", None);
                        // Build a reset string:
                        // if multiple value display the first two, and an ellipsis if at least a third value exists
                        let resets : Vec<ResetVal> = reg_insts.iter().filter_map(|reg_inst| {
                                if let Some(f_inst) = reg_inst.find_field(&f.name, f.array.idx()) {
                                    let reg_def_idx = if f.array.dim() == 0 {reg_inst.def_idx()} else {None};
                                    Some(f_inst.reset_val(reg_def_idx))
                                } else {
                                    // Can happen only in case of field array in a register array
                                    // Print a clear message if this happens in other circumstances to debug library
                                    if !(reg_inst.array.is_def() && reg_inst.array.idx() > 0) {
                                        eprintln!("[ERROR] Field {fieldname} == {} with index {:?} (reg {:?}): Unable to find amongst {:?}",
                                            f.name, f.array, reg_inst.array, reg_inst.fields.iter().map(|fi| (&fi.name, fi.array.clone())).collect::<Vec<_>>());
                                    }
                                    None
                                }
                            }).collect();
                        let reg_def_idx = if f.array.dim() == 0 {reg.def_idx()} else {None};
                        let mut rst = f.reset_str(reg_def_idx);
                        let f0 = resets.first().cloned().unwrap_or_default();
                        let mut f_inst_reset = f.reset_val(reg_def_idx);
                        for inst_rst in resets.iter().skip(1) {
                            if *inst_rst != f0 && *inst_rst != f_inst_reset {
                                rst.push('/');
                                if f_inst_reset == f0 {
                                    rst.push_str(&val_str(inst_rst.to_u128(f.width), f.width.into(), f.is_signed(), reg_def_idx));
                                    f_inst_reset = inst_rst.clone();
                                } else {
                                    rst.push('…');
                                    break;
                                }
                            }
                        }
                        // if resets.len() > 1 {println!("Field {fieldname} : {resets:?}");}
                        let tip = if f.nb_frac!=0 {
                            let prec = if f.nb_frac < 0 {0} else {f.nb_frac as usize};
                            Some(resets.iter().map(|r| {
                                if f.nb_frac > 4 {
                                    format!("{:.3e}", r.to_f64(f.nb_frac))
                                } else {
                                    format!("{:.prec$}", r.to_f64(f.nb_frac))
                                }
                            }).collect::<Vec<String>>().join("\n"))
                        } else if rst.ends_with('…') {
                            Some(resets.iter().map(|r|
                                val_str(r.to_u128(f.width), f.width.into(), f.is_signed(), reg_def_idx)
                            ).collect::<Vec<String>>().join("\n"))
                        } else {
                            None
                        };
                        self.write_table_cell((TableKind::Field, CellKind::Reset), 0, &rst, "", tip);
                        // Description
                        let mut desc = self.sanitize(&f.base_description.get(self.is_public()));
                        if let Some(enum_name) = f.enum_kind.name() {
                            let name = if let Some(pkg) = &reg_impl.pkg {
                                if enum_name.contains(':') {enum_name.to_owned()}
                                else {format!("{pkg}_pkg::{enum_name}")}
                            } else {
                                enum_name.to_owned()
                            };
                            let enum_def = rif.get_enum_def(&name)?;
                            if !desc.is_empty() {
                                desc.push_str(&self.sanitize("\n"));
                            }
                            desc.push_str(&self.enum_def_desc(enum_def));
                        }
                        // Check for partial disable in an array
                        let dis : Vec<u16> = reg_insts.iter().filter_map(|reg_inst| {
                            if let Some(f) = reg_inst.find_field(&f.name, f.array.idx()) {
                                if f.is_disabled() {Some(reg_inst.array.idx())} else {None}
                            } else {
                                None
                            }
                        }).collect();
                        if !dis.is_empty() && reg_insts.len() > 1 && dis.len() != reg_insts.len() {
                            desc.push_str(&self.sanitize(&format!(" \nNote: Disabled in register {dis:?}")));
                        }
                        // Check for field array not exisiing in some register instances
                        if !reg.array.is_inst() && reg.array.dim() > 1 && f.array.dim() > 0
                            && let Ok(field_impl) = reg_impl.get_field(&f.name) {
                                let reg_idx = reg.array.dim() - 1;
                                let last_field_idx = reg_idx * f.array.dim() + f.array.idx();
                                if last_field_idx >= field_impl.array {
                                    desc.push_str(&self.sanitize(&format!(" \nNote: Disabled in register {}[{reg_idx}]", reg_impl.name)));
                                }
                        }
                        self.write_table_cell((TableKind::Field, CellKind::Desc), 0, &desc, "", None);
                        self.write_table_row_footer();
                        last_pos = f.lsb;
                    }
                    if Self::SHOW_UNUSED && last_pos!=0 {
                        let pos = if last_pos==0 {"0".to_owned()} else {format!("{}:0", last_pos-1)};
                        self.write_table_row_header(None);
                        self.write_table_cell((TableKind::FieldRsvd, CellKind::Bits  ), 0, &pos , "", None);
                        self.write_table_cell((TableKind::FieldRsvd, CellKind::Inst  ), 0, ""   , "", None);
                        self.write_table_cell((TableKind::FieldRsvd, CellKind::Access), 0, "RO" , "", None);
                        self.write_table_cell((TableKind::FieldRsvd, CellKind::Reset ), 0, "0x0", "", None);
                        self.write_table_cell((TableKind::FieldRsvd, CellKind::Desc  ), 0, ""   , "", None);
                        self.write_table_row_footer();
                    }
                    self.write_table_footer(TableKind::Field);
                }
                self.add_link(LinkKind::Page, &id_page);
                if is_intr_derived {
                    self.add_link(LinkKind::Field, &format!("fields.{id_reg}"));
                }
                self.write("\n");
            }
        }
        Ok(())
    }

    /// Write header for the register detail tables
    fn write_reg_detail_header(&mut self, rif: &RifInst) {}

    /// Convert access to string
    fn access_str(kind: &FieldSwKind) -> &str {
        kind.access_str()
    }

    /// Sanitize a text from all special characters
    /// Default implementation provides a simple copy
    fn sanitize(&self, raw: &str) -> String {
        raw.to_owned()
    }

    /// Set the width of address/data for current RIF
    fn set_rif_info(&mut self, rif: &RifInst) {}

    /// Write file header
    /// No header by default
    fn write_header(&mut self, name: &str) {}

    /// Write file footer (empty by default)
    fn write_footer(&mut self, name: &str) {}

    /// Write component title
    fn write_rif_title(&mut self, idx_rif: (&str,usize), desc: &str) {}

    /// Write page title
    fn write_page_title(&mut self, idx_rif: (&str,usize), idx_page: (&str, usize), desc: (String, Option<String>)) {}

    /// Write register title
    fn write_reg_title(&mut self, idx_rif: (&str,usize), idx_page: (&str, usize), idx_reg: (&str, usize), desc: &str, base_addr: Option<u64>) {}

    /// Write component/page information
    fn write_info(&mut self, info: &str) {
        self.core_mut().write(info)
    }

    /// Write a table header
    fn write_table_title(&mut self, kind: TableKind, title: &str, id: &str) {}

    /// Write a table header
    fn write_table_footer(&mut self, kind: TableKind) {}

    /// Write a table row header
    fn write_table_row_header(&mut self, id: Option<&str>) {}

    /// Write a table row footer
    fn write_table_row_footer(&mut self) {}

    /// Write a table top row header
    fn write_table_row_top_header(&mut self, kind: TableKind) {
        self.write_table_row_header(None)
    }

    /// Write a table top row footer
    fn write_table_row_top_footer(&mut self) {
        self.write_table_row_footer()
    }

    /// Write a table column
    fn write_table_cell(&mut self, kind: (TableKind, CellKind), span: usize, txt: &str, id: &str, tip: Option<String>) {}

    /// Write a table cell header
    fn write_table_cell_top(&mut self, kind: (TableKind, CellKind), span: usize, txt: &str, id: &str) {
        self.write_table_cell(kind, span, txt, id, None);
    }

    /// Add a link to an ID of the document
    fn add_link(&mut self, kind: LinkKind, _id: &str) {}

    /// Write description of an enum field
    fn enum_def_desc(&mut self, def: &EnumDef) -> String;
}
