use std::collections::{HashMap, HashSet};
use std::env::current_dir;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde_derive::Deserialize;
use winnow::Parser;

use crate::error::{RifError, RifErrorKind, ERROR_CONTEXT};
use crate::parser::parser_expr::parse_expr;
use crate::parser::{
    bool_or_default, clk_en, enum_kind, generic_def, intr_desc, is_hidden, limit_def, password_info, path_val,
    reg_incl_or_decl, reg_inst_array_properties, reg_inst_properties, reg_pulse_info, rif_inst_suffix, rifmux_group, rifmux_map, signal_or_expr, val_isize, val_u16
};
use crate::rifgen::{
    Access, AddressOffset, ClockingInfo, Context, DataWidth, EnumDef, EnumKind, ExternalKind, Field, FieldHwKind, FieldSwKind, InstMode, Interface, InterruptInfo, Lock, LogicExpr, OverrideIndex, RegDef, RegDefOrIncl, RegInst, RegPulseKind, ResetDef, Rif, RifPage, RifType, Rifmux, RifmuxItem, RifmuxTop, Visibility
};

use super::{
    comment, counter_def, decl_top, desc, enum_entry, field_decl, field_acc, field_interrupt,
    field_properties, identifier, identifier_last, indentation, is_auto, key_val,
    opt_signal_or_expr, page_properties, pulse_kind, reg_decl, reg_inst,
    reg_inst_field_properties, reg_interrupt, reg_properties_or_item, reset_def, reset_val,
    rif_inst, rif_inst_properties, rif_properties_or_item, rifmux_properties,
    val_intf, val_u64, val_u8, vec_id
};

#[derive(Clone, Debug, PartialEq, Default)]
pub enum RifGenTop {#[default]
    None,
    Rifmux(String),
    Rif(String),
}

#[derive(Debug)]
/// Source RIF Generator (result of parsing)
pub struct RifGenSrc {
    /// Top level
    pub top: RifGenTop,
    /// Dictionary of all rif definitions parsed
    pub rifs: HashMap<String, Rif>,
    /// Dictionary of all rifmux parsed
    pub rifmux: HashMap<String, Rifmux>,
    /// Path of each rif/rifmux
    pub paths: HashMap<String, PathBuf>,
    // Internal state used for parsgin
    last_hidden: bool,
    last_data_width: DataWidth,
    last_obj: String,
    last_group: String,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub struct RsvdKeywordSel {
    /// Prevent use of a list of SystemVerilog Keyword
    pub sv: bool,
    /// Prevent use of a list of VHDL Keyword
    pub vhdl: bool,
    /// Generate an error if a field matches a reserved keyword.
    /// Otherwise simply generate a warning and prefix name with underscore
    pub error: bool,
}

impl Default for RsvdKeywordSel {
    fn default() -> Self {
        RsvdKeywordSel {sv: true, vhdl: false, error: true}
    }
}

#[derive(Debug, Clone)]
pub struct ParserCfg {
    pub rsvd_kw: Vec<&'static str>,
    pub rsvd_err: bool,
    pub auto_legacy: bool,
}

impl ParserCfg {
    pub fn new(rsvd_sel: RsvdKeywordSel, auto_legacy: bool) -> Self {
        let mut rsvd_kw = Vec::new();
        if rsvd_sel.sv {
            rsvd_kw.extend(["logic", "signed", "wire", "reg", "buf","event", "soft", "break", "module", "process", "type", "priority", "disable", "longint", "int", "release", "repeat", "if", "always", "default"]);
        }
        if rsvd_sel.vhdl {
            rsvd_kw.extend(["array", "buffer", "map", "out", "in", "on","generic", "block", "type", "rem", "if"]);
        }
        Self {
            rsvd_kw, rsvd_err: rsvd_sel.error, auto_legacy
        }
    }
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(BufReader::new(file).lines())
}

type ContextStack = Vec<(Context, usize)>;

impl Default for RifGenSrc {
    fn default() -> Self {
        Self::new()
    }
}

impl RifGenSrc {
    pub fn new() -> RifGenSrc {
        RifGenSrc {
            top: RifGenTop::None,
            rifs: HashMap::new(),
            rifmux: HashMap::new(),
            paths: HashMap::new(),
            last_hidden: false,
            last_data_width: DataWidth::default(),
            last_obj: "".to_owned(),
            last_group: "".to_owned(),
        }
    }

    /// Generate a source object from a file
    /// the includes path are used to find and parse any included/referenced RIF
    /// The configuration contains list of reserved keywords and how they should be handled, as well as as setting such as auto-legacy
    pub fn from_file<P>(filename: P, includes: &[String], cfg: &ParserCfg) -> Result<RifGenSrc, RifError>
    where
        P: AsRef<Path>,
    {
        let mut src = RifGenSrc::new();
        let mut refs = src.parse_file(&filename, cfg)?;
        if !refs.is_empty() {
            // find all rifs file in current directory and import directories
            let mut inc_paths = Vec::new();
            let full_path = filename.as_ref().canonicalize()?;
            if let Some(cwd) = full_path.parent() {
                inc_paths.push(cwd.to_owned());
            }
            inc_paths.extend(includes.iter().map(|p| p.into()));
            let mut flist: HashMap<String, PathBuf> = HashMap::new();
            for path in inc_paths.iter() {
                let Ok(files) = fs::read_dir(path) else {
                    eprintln!("Unable to read include dir '{path:?}'");
                    continue;
                };
                flist.extend(
                    files.filter(|p| {
                        p.as_ref()
                            .unwrap()
                            .path()
                            .extension()
                            .map(|s| s == "rif")
                            .unwrap_or(false)
                    })
                    .map(|p| {
                        let path = p.unwrap().path();
                        let rifname = remove_rif(path.file_stem().unwrap().to_str().unwrap());
                        (rifname.to_owned(), path)
                    }));
            };
            let mut ref_done = false;
            while !ref_done {
                // print!(" , Files = {:#?} ", flist);
                let mut refs_next: HashSet<String> = HashSet::new();
                for r in refs.iter() {
                    if let Some(rif_file) = flist.get(remove_rif(r)) {
                        // println!("  Parsing referenced {:?}", rif_file);
                        refs_next.extend(src.parse_file(rif_file, cfg)?);
                    }
                }
                // print!(" => New refs = {:?} ", refs_next);
                refs = refs_next;
                ref_done = refs.is_empty();
            }
        }
        Ok(src)
    }

    pub fn parse_file<P>(&mut self, filename: P, cfg: &ParserCfg) -> Result<HashSet<String>, RifError>
    where
        P: AsRef<Path>,
    {
        let mut refs = HashSet::new();
        if let Some(file_path) = filename.as_ref().file_name() {
            err_set_name!(file_path.to_string_lossy().to_string());
        }
        let filedir : PathBuf = filename.as_ref()
            .parent()
            .expect("Filename should have a parent")
            .to_path_buf();
        let filedir = if filedir.as_os_str().is_empty() {current_dir()?} else {filedir.canonicalize()?};
        let mut lines = read_lines(filename)?;
        let mut context_stack: ContextStack = vec![(Context::Top, 0)];
        let mut line_num = 0;
        let mut desc_lvl = 0;
        let mut last_enum : Option<String> = None;
        let mut ovr_idx = OverrideIndex::default();
        let mut sw_clk_defined = (false,false);
        while let Some(Ok(l)) = lines.next() {
            let mut l = l.as_str();
            line_num += 1;
            // Skip comment line
            if comment(l).is_ok() {
                continue;
            }
            if l.is_empty() {
                continue;
            }
            // Check indentation level To update the context
            let ilvl = indentation(&mut l)?;
            while ilvl < context_stack.last().expect("Context Stack Empty").1 {
                if let Some(cntxt) = context_stack.pop() && cntxt.0 == Context::RifmuxGroup {
                    self.last_group = "".to_string();
                }
            }
            let cntxt = context_stack.last().expect("Context Stack Empty !");
            err_set_context!(line_num, cntxt.0.to_owned());
            // Call parsers based on context
            match cntxt.0 {
                // Parse Top level declaration: either Rif or Rifmux
                Context::Top => match decl_top(&mut l)? {
                    (Context::Rif, name) => {
                        if self.top == RifGenTop::None {
                            self.top = RifGenTop::Rif(name.to_owned());
                        }
                        self.last_obj = name.to_owned();
                        let rif = Rif::new(name);
                        self.rifs.insert(name.to_owned(), rif);
                        self.paths.insert(remove_rif(name).to_owned(), filedir.clone());
                        self.last_data_width = DataWidth::default();
                        context_stack.push((Context::Rif, ilvl));
                    }
                    (Context::Rifmux, name) => {
                        if self.top == RifGenTop::None {
                            self.top = RifGenTop::Rifmux(name.to_owned());
                        }
                        self.last_obj = name.to_owned();
                        let rifmux = Rifmux::new(name);
                        self.rifmux.insert(name.to_owned(), rifmux);
                        self.paths.insert(remove_rif(name).to_owned(), filedir.clone());
                        self.last_data_width = DataWidth::default();
                        context_stack.push((Context::Rifmux, ilvl));
                    }
                    (info, _) => {
                        return Err(RifError::unsupported(info, l));
                    }
                },
                // Parse properties of RIF
                Context::Rif => {
                    let info = rif_properties_or_item(&mut l)?;
                    match info {
                        Context::Description => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_rif().description.updt(desc(l)?, hidden);
                            }
                            context_stack.push((Context::Description, ilvl + 1));
                            desc_lvl = 0;
                        }
                        Context::Parameters => context_stack.push((Context::Parameters, ilvl + 1)),
                        Context::Info => context_stack.push((Context::Info, ilvl + 1)),
                        Context::Interface => {
                            let intf = val_intf(&mut l)?;
                            if intf == Interface::Apb {
                                if !sw_clk_defined.0 {self.last_rif().sw_clocking.clk = "pclk".to_owned();}
                                if !sw_clk_defined.1 {self.last_rif().sw_clocking.rst = ResetDef::new("presetn".to_owned());}
                            }
                            self.last_rif().interface = intf;
                        }
                        Context::AddrWidth => self.last_rif().addr_width = val_u8(&mut l)?,
                        Context::DataWidth => {
                            let w = val_u8(&mut l)?.try_into()?;
                            self.last_rif().data_width = w;
                            self.last_data_width = w;
                        }
                        Context::SwClock => {
                            sw_clk_defined.0 = true;
                            self.last_rif().sw_clocking.clk = identifier_last(l)?.to_owned()
                        }
                        Context::SwClkEn => {
                            self.last_rif().sw_clocking.en = identifier_last(l)?.to_owned()
                        }
                        Context::SwReset => {
                            sw_clk_defined.1 = true;
                            self.last_rif().sw_clocking.rst = reset_def(l)?;
                        }
                        Context::SwClear => {
                            self.last_rif().sw_clocking.clear = identifier_last(l)?.to_owned()
                        }
                        Context::HwClock => self.last_rif().set_hw_clk(vec_id(l)?),
                        Context::HwClkEn => self.last_rif().set_hw_clken(vec_id(l)?),
                        Context::HwReset => self.last_rif().set_hw_rst(reset_def(l)?),
                        Context::HwClear => self.last_rif().set_hw_clear(vec_id(l)?),
                        Context::SuffixPkg => {
                            self.last_rif().suffix_pkg = bool_or_default(l, false)?
                        }
                        Context::Generics => context_stack.push((Context::Generics, ilvl + 1)),
                        Context::Item(name) => {
                            self.last_rif().pages.push(RifPage::new(name));
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_page_mut().description.updt(desc(l)?, hidden);
                            }
                            context_stack.push((Context::Page, ilvl + 1));
                        }
                        _ => {
                            return Err(RifError::unsupported(info, l));
                        }
                    }
                }
                Context::Parameters => {
                    let prev_cntxt = context_stack.get(context_stack.len() - 2);
                    let (k,v) =
                        if matches!(prev_cntxt, Some((Context::RifInst, _))) {path_val(l)}
                        else {key_val(l)}
                    ?;
                    let expr = parse_expr(v)?;
                    match prev_cntxt {
                        Some((Context::Rifmux, _))  => {
                            let last_rifmux = self.last_rifmux();
                            if last_rifmux.generics.contains_key(k) {
                                return Err(RifError::duplicated(Context::Parameters, k));
                            }
                            last_rifmux.add_param(k,expr);
                        }
                        Some((Context::Rif, _))     => {
                            let last_rif = self.last_rif();
                            if last_rif.generics.contains_key(k) {
                                return Err(RifError::duplicated(Context::Parameters, k));
                            }
                            last_rif.add_param(k,expr);
                        }
                        Some((Context::RifInst, _)) => self.last_rif_inst().add_param(k,expr),
                        _ => unreachable!(), // Should never fail
                    }
                }
                Context::Generics => {
                    let gen_def = generic_def(l)?;
                    let prev_cntxt = context_stack.get(context_stack.len() - 2);
                    match prev_cntxt {
                        Some((Context::Rifmux, _)) => {
                            let last_rifmux = self.last_rifmux();
                            if last_rifmux.parameters.contains_key(gen_def.0) {
                                return Err(RifError::duplicated(Context::Generics, gen_def.0));
                            }
                            last_rifmux.add_generic(gen_def);
                        }
                        Some((Context::Rif, _)) => {
                            let last_rif = self.last_rif();
                            if last_rif.parameters.contains_key(gen_def.0) {
                                return Err(RifError::duplicated(Context::Generics, gen_def.0));
                            }
                            last_rif.add_generic(gen_def);
                        }
                        _ => unreachable!(), // Should never fail
                    }
                },
                // Parse page properties: register definition or instance
                Context::Page => {
                    let info = page_properties(&mut l)?;
                    match info {
                        Context::BaseAddress => self.last_page_mut().addr = val_u64(&mut l)?,
                        Context::Description => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_page_mut().description.updt(desc(l)?, hidden);
                            }
                            context_stack.push((Context::Description, ilvl + 1));
                            desc_lvl = 0;
                        }
                        Context::Registers => {
                            context_stack.push((Context::Registers, ilvl + 1));
                        }
                        Context::Instances => {
                            let mut inst_auto = is_auto(l)?;
                            if cfg.auto_legacy && inst_auto==InstMode::Automatic {
                                inst_auto = InstMode::AutoLegacy;
                            }
                            self.last_page_mut().inst_auto = inst_auto;
                            context_stack.push((Context::Instances, ilvl + 1));
                        }
                        Context::Optional => self.last_page_mut().optional = l.to_owned(),
                        Context::External => {
                            self.last_page_mut().external = true;
                            if !l.is_empty() {
                                self.last_page_mut().addr_width = val_u8.parse(l)?;
                            }
                        }
                        Context::AddrWidth => {
                            self.last_page_mut().addr_width = val_u8.parse(l)?
                        }
                        Context::HwClkEn => {
                            self.last_page_mut().clk_en = clk_en(l)?;
                        }
                        _ => {
                            return Err(RifError::unsupported(info, l));
                        }
                    }
                }
                // Registers
                Context::Registers => {
                    let info = reg_incl_or_decl(&mut l)?;
                    match info {
                        Context::Include => {
                            self.last_page_mut()
                                .registers
                                .push(RegDefOrIncl::Include(l.to_owned()));
                            refs.insert(identifier(&mut l)?.to_owned());
                        }
                        Context::Registers => {
                            let r = reg_decl(l)?;
                            if !self.check_reg_uniq(&r.name) {
                                return Err(RifError::duplicated(info, &r.name));
                            }
                            if let Some(rif_name) = &r.group.pkg {
                                refs.insert(rif_name.to_owned());
                            }
                            self.last_page_mut().registers.push(RegDefOrIncl::Def(Box::new(r)));
                            context_stack.push((Context::RegDecl, ilvl + 1));
                        }
                        _ => {
                            return Err(RifError::unsupported(info, l));
                        }
                    }
                }
                Context::RegDecl => {
                    let info = reg_properties_or_item(&mut l)?;
                    match info {
                        Context::Info => context_stack.push((Context::Info, ilvl + 1)),
                        Context::Description => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_reg_mut().description.updt(desc(l)?, hidden);
                            }
                            context_stack.push((Context::Description, ilvl + 1));
                            desc_lvl = 0;
                        }
                        Context::DescIntrEnable
                        | Context::DescIntrMask
                        | Context::DescIntrPending => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_reg_mut().desc_intr_updt(&info, "", desc(l)?, hidden)?;
                            }
                            context_stack.push((info, ilvl + 1));
                        }
                        Context::PathStart(name) => {
                            let info_desc = intr_desc(&mut l)?;
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_reg_mut().desc_intr_updt(&info_desc, &name, desc(l)?, hidden)?;
                            }
                            context_stack.push((info_desc, ilvl + 1));
                        }
                        Context::HwClock => self.last_reg_mut().clk = Some(identifier_last(l)?.to_owned()),
                        Context::HwClkEn => self.last_reg_mut().clk_en = clk_en(l)?,
                        Context::HwClear => {
                            let expr = opt_signal_or_expr(l)?;
                            let clear = if expr.is_none() {
                                Some(LogicExpr::Id(format!("{}_clr",self.last_reg().name).into()))
                            } else {
                                expr
                            };
                            self.last_reg_mut().clear = clear;
                        }
                        Context::HwReset => {
                            self.last_reg_mut().rst = Some(identifier_last(l)?.to_owned())
                        }
                        Context::External => self.last_reg_mut().external = ExternalKind::ReadWrite,
                        Context::ExternalDone => self.last_reg_mut().external = ExternalKind::Done,
                        Context::RegPulseWr => {
                            let n = reg_pulse_info(&mut l, &self.last_rif().sw_clocking.clk, true)?;
                            self.last_reg_mut().pulse.push(RegPulseKind::Write(n));
                        },
                        Context::RegPulseRd => {
                            let n = reg_pulse_info(&mut l, &self.last_rif().sw_clocking.clk, false)?;
                            self.last_reg_mut().pulse.push(RegPulseKind::Read(n));
                        },
                        Context::RegPulseAcc => {
                            let n = reg_pulse_info(&mut l, &self.last_rif().sw_clocking.clk, false)?;
                            self.last_reg_mut().pulse.push(RegPulseKind::Access(n));
                        },
                        Context::Interrupt => {
                            let info = reg_interrupt(&mut l)?;
                            self.last_reg_mut().interrupt.push(InterruptInfo::new("", info));
                        },
                        Context::InterruptAlt => {
                            let name = identifier(&mut l)?;
                            let mut info = reg_interrupt(&mut l)?;
                            // Inherit from first defined interrupt
                            if let Some(intf_def) = self.last_reg().interrupt.first() {
                                if info.0.is_none() {info.0 = Some(intf_def.trigger);}
                                if info.1.is_none() {info.1 = Some(intf_def.clear);}
                                if info.2.is_none() {info.2 = intf_def.enable.clone();}
                                if info.3.is_none() {info.3 = intf_def.mask.clone();}
                                if info.4.is_none() {info.4 = Some(intf_def.pending);}
                            }
                            self.last_reg_mut().interrupt.push(InterruptInfo::new(name, info));
                        },
                        Context::Optional => self.last_reg_mut().optional = l.to_owned(),
                        Context::OptionalAcc => self.last_reg_mut().optional_acc = field_acc(&mut l)?,
                        Context::Hidden => self.last_reg_mut().hidden(),
                        Context::Reserved => self.last_reg_mut().reserved(),
                        Context::Item(_) => {
                            let mut f = field_decl(&mut l)?;
                            if !self.last_reg().interrupt.is_empty() {
                                f.hw_acc = Access::WO;
                            }
                            if cfg.rsvd_kw.iter().any(|&kw| f.name==kw) {
                                if cfg.rsvd_err {
                                    return Err(RifError::keyword(&f.name));
                                } else {
                                    f.name = format!("_{}", f.name);
                                    println!("[WARNING] Field name {} is a reserved keyword !", f.name);
                                }
                            }
                            self.last_reg_mut().add_field(f);
                            context_stack.push((Context::Field, ilvl + 1));
                        }
                        _ => {
                            return Err(RifError::unsupported(info, l));
                        }
                    }
                }
                // Fields properties
                Context::Field => {
                    let info = field_properties(&mut l)?;
                    match info {
                        Context::Description => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_field_mut().description.updt(desc(l)?, hidden);
                            }
                            context_stack.push((Context::Description, ilvl + 1));
                            desc_lvl = 0;
                        }
                        Context::DescIntrEnable
                        | Context::DescIntrMask
                        | Context::DescIntrPending => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_field_mut().desc_intr_updt(&info, desc(l)?, hidden);
                            }
                            context_stack.push((info, ilvl + 1));
                        }
                        Context::HwClock => self.last_field_mut().clk = Some(identifier_last(l)?.to_owned()),
                        Context::HwClkEn => self.last_field_mut().clk_en = clk_en(l)?,
                        Context::HwClear => {
                            let expr = opt_signal_or_expr(l)?;
                            let clear = if expr.is_none() {
                                Some(LogicExpr::Id(format!("{}_clr",self.last_field().name).into()))
                            } else {
                                expr
                            };
                            self.last_field_mut().clear = clear;
                        }
                        Context::HwAccess => self.last_field_mut().set_hw_acc(field_acc(&mut l)?),
                        Context::HwSet => {
                            self.last_field_mut()
                                .set_hw_kind(FieldHwKind::Set(opt_signal_or_expr(l)?.map(|v| v.to_owned())))?;
                        }
                        Context::HwClr => {
                            self.last_field_mut()
                                .set_hw_kind(FieldHwKind::Clear(opt_signal_or_expr(l)?.map(|v| v.to_owned())))?;
                        }
                        Context::HwTgl => {
                            self.last_field_mut()
                                .set_hw_kind(FieldHwKind::Toggle(opt_signal_or_expr(l)?.map(|v| v.to_owned())))?;
                        }
                        Context::HwLock => {
                            self.last_field_mut().lock = Lock::new(signal_or_expr(l)?.to_owned())
                        }
                        Context::Pulse => {
                            let wo = self.last_field_mut().sw_kind==FieldSwKind::WriteOnly;
                            self.last_field_mut()
                                .set_sw_kind(FieldSwKind::W1Pulse(pulse_kind(l)?, wo))?;
                        }
                        Context::Toggle => {
                            self.last_field_mut().set_sw_kind(FieldSwKind::W1Tgl)?;
                        }
                        Context::Password => {
                            self.last_field_mut().set_sw_kind(FieldSwKind::Password(password_info(l)?))?;
                        }
                        Context::Interrupt => {
                            self.last_field_mut().set_intr(field_interrupt(&mut l)?);
                        }
                        Context::SwSet => {
                            return Err(RifError::unsupported(info, l));
                        }
                        Context::Signed => {
                            self.last_field_mut().set_signed();
                        }
                        Context::HwWe => {
                            self.last_field_mut().set_hw_kind(FieldHwKind::WriteEn(
                                opt_signal_or_expr(l)?.map(|v| v.to_owned()),
                            ))?;
                        }
                        Context::HwWel => {
                            self.last_field_mut().set_hw_kind(FieldHwKind::WriteEnL(
                                opt_signal_or_expr(l)?.map(|v| v.to_owned()),
                            ))?;
                        }
                        Context::Counter => {
                            self.last_field_mut()
                                .set_hw_kind(FieldHwKind::Counter(counter_def(l)?))?;
                        }
                        Context::Partial => self.last_field_mut().partial.0 = Some(val_u16(&mut l)?),
                        Context::Hidden => self.last_field_mut().hidden(),
                        Context::Reserved => self.last_field_mut().reserved(),
                        Context::Disabled => self.last_field_mut().disabled(parse_expr(l)?),
                        Context::Optional => self.last_field_mut().optional = l.to_owned(),
                        Context::ArrayPosIncr => self.last_field_mut().array_pos_incr = val_u8(&mut l)?,
                        Context::ArrayPartial => self.last_field_mut().partial.1 = val_u16(&mut l)?,
                        Context::Enum => {
                            let regname = self.last_reg().get_group_name().to_owned();
                            let enum_kind = EnumKind::new( enum_kind(&mut l)?, &regname, &self.last_field_mut().name);
                            let mut desc = (desc(l)?).to_owned();
                            if let Some(enum_name) = enum_kind.name() {
                                if !self.last_rif().enum_defs.iter().any(|d| d.name==enum_name) {
                                    // set the enum description to the one from the field if none was provided
                                    if desc.is_empty() {
                                        desc = self.last_field_mut().description.get_short(true);
                                    }
                                    let enum_def = EnumDef::new(enum_name.to_owned(), desc);
                                    last_enum = Some(enum_def.name.to_owned());
                                    self.last_rif().enum_defs.push(enum_def);
                                    context_stack.push((Context::Enum, ilvl + 1));
                                } else {
                                    last_enum = None;
                                }
                            }
                            self.last_field_mut().enum_kind = enum_kind;
                        }
                        Context::Limit => self.last_field_mut().limit = limit_def(l)?,
                        Context::FieldFrac => self.last_field_mut().nb_frac = val_isize(&mut l)?,
                        _ => {
                            return Err(RifError::unsupported(info, l));
                        }
                    }
                }
                // Description
                Context::Description => {
                    let mut txt = String::with_capacity(l.len());
                    if desc_lvl==0 {
                        desc_lvl = ilvl;
                    } else if ilvl > desc_lvl {
                        txt.push_str(&" ".repeat(ilvl - desc_lvl));
                    }
                    txt.push_str(desc(l)?);
                    let hidden = self.last_hidden;
                    match context_stack.get(context_stack.len() - 2) {
                        Some((Context::Rifmux, _))  => self.last_rifmux().description.updt(&txt, hidden),
                        Some((Context::Rif, _))     => self.last_rif().description.updt(&txt, hidden),
                        Some((Context::Page, _))    => self.last_page_mut().description.updt(&txt, hidden),
                        Some((Context::RegDecl, _)) => self.last_reg_mut().description.updt(&txt, hidden),
                        Some((Context::Field, _))   => self.last_field_mut().description.updt(&txt, hidden),
                        Some((Context::RifInst, _)) => self.last_rif_inst().description.updt(&txt, hidden),
                        Some((Context::RegInst, _)) => self.last_reg_inst().desc_updt(&ovr_idx, &txt, hidden),
                        _ => unreachable!(), // Should never fail
                    }
                }
                Context::DescIntrEnable |
                Context::DescIntrMask |
                Context::DescIntrPending => {
                    let hidden = self.last_hidden;
                    match context_stack.get(context_stack.len() - 2) {
                        Some((Context::RegDecl, _)) => self.last_reg_mut().desc_intr_updt(&cntxt.0, "", desc(l)?, hidden)?,
                        Some((Context::Field, _))   => self.last_field_mut().desc_intr_updt(&cntxt.0, desc(l)?, hidden),
                        _ => unreachable!(), // Should never fail
                    }

                }
                Context::Info => {
                    match context_stack.get(context_stack.len() - 2) {
                        Some((Context::Rifmux, _)) => self.last_rifmux().add_info(key_val(l)?),
                        Some((Context::Rif, _)) => self.last_rif().add_info(key_val(l)?),
                        // Some((Context::Page,_))    => parser.last_page().add_info(key_val(l)?),
                        Some((Context::RegDecl, _)) => self.last_reg_mut().add_info(key_val(l)?),
                        Some((Context::RegInst, _)) => self.last_reg_inst().add_info(&ovr_idx, key_val(l)?),
                        c => unreachable!("{:?}", c), // Should never fail
                    }
                }
                // Enum definition
                Context::Enum => {
                    if let Some(name) = &last_enum {
                        let entry = enum_entry(l)?;
                        self.last_rif().add_enum_entry(name, entry)?;
                    }
                }
                // Instances
                Context::Instances => {
                    let inst = reg_inst(l)?;
                    if let AddressOffset::Value(addr) = &inst.addr.offset && (addr & self.last_data_width.addr_mask()) != 0 {
                        return Err(RifErrorKind::AddrUnaligned.into())
                    }
                    self.last_page_mut().instances.push(inst);
                    context_stack.push((Context::RegInst, ilvl + 1));
                }
                // Parse properties of RIF
                Context::Rifmux => {
                    let info = rifmux_properties(&mut l)?;
                    match info {
                        Context::Description => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_rifmux().description.updt(desc(l)?, hidden);
                            }
                            context_stack.push((Context::Description, ilvl + 1));
                        }
                        Context::Info => context_stack.push((Context::Info, ilvl + 1)),
                        Context::Interface => {
                            let intf = val_intf(&mut l)?;
                            // Default clock/reset for APB
                            if intf == Interface::Apb {
                                if !sw_clk_defined.0 {self.last_rifmux().sw_clocking.clk = "pclk".to_owned();}
                                if !sw_clk_defined.1 {self.last_rifmux().sw_clocking.rst = ResetDef::new("presetn".to_owned());}
                            }
                            self.last_rifmux().interface = intf;
                        }
                        Context::AddrWidth => self.last_rifmux().addr_width = val_u8(&mut l)?,
                        Context::DataWidth => {
                            let w = val_u8(&mut l)?.try_into()?;
                            self.last_rifmux().data_width = w;
                            self.last_data_width = w;
                        }
                        Context::Parameters => context_stack.push((Context::Parameters, ilvl + 1)),
                        Context::Generics => context_stack.push((Context::Generics, ilvl + 1)),
                        Context::SwClock => {
                            sw_clk_defined.0 = true;
                            self.last_rifmux().sw_clocking.clk = identifier_last(l)?.to_owned()
                        }
                        Context::SwClkEn => {
                            self.last_rifmux().sw_clocking.en = identifier_last(l)?.to_owned()
                        }
                        Context::SwReset => {
                            sw_clk_defined.1 = true;
                            self.last_rifmux().sw_clocking.rst = reset_def(l)?;
                        }
                        Context::RifmuxMap => context_stack.push((Context::RifmuxMap, ilvl + 1)),
                        Context::RifmuxTop => {
                            self.last_rifmux().top = Some(RifmuxTop::new(identifier_last(l)?));
                            context_stack.push((Context::RifmuxTop, ilvl + 1))
                        }
                        _ => {
                            return Err(RifError::unsupported(info, l));
                        }
                    }
                }
                Context::RifmuxMap |
                Context::RifmuxGroup => {
                    let info = rifmux_map(&mut l)?;
                    match info {
                        Context::Item(_) => {
                            let r = rif_inst(l, &self.last_group)?;
                            if let RifType::Rif(n) = &r.rif_type {
                                refs.insert(n.to_owned());
                            }
                            self.last_rifmux().items.push(r);
                            context_stack.push((Context::RifInst, ilvl + 1));
                        }
                        Context::RifmuxGroup => {
                            let group = rifmux_group(l)?;
                            self.last_group = group.name.clone();
                            self.last_rifmux().groups.push(group);
                            context_stack.push((Context::RifmuxGroup, ilvl + 1));
                        },
                        _ => return Err(RifError::unsupported(info, l)),
                    }
                }
                Context::RifmuxTop => {
                    let (key,val) = key_val(l)?;
                    self.last_rifmux().add_top_suffix(key, val);
                }
                Context::RegInst => {
                    ovr_idx.clear();
                    let info = reg_inst_properties(&mut l)?;
                    match info {
                        Context::Description => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_reg_inst().desc_updt(&ovr_idx, desc(l)?, hidden);
                            }
                            context_stack.push((Context::Description, ilvl + 1));
                        }
                        Context::Optional => {
                            self.last_reg_inst().set_optional(&ovr_idx, parse_expr(l)?);
                        }
                        Context::OptionalAcc => {
                            self.last_reg_inst().set_optional_acc(&ovr_idx, field_acc(&mut l)?);
                        }
                        Context::HwAccess => self
                            .last_reg_inst()
                            .set_hw_acc(&ovr_idx, field_acc(&mut l)?),
                        Context::Hidden => {
                            let v = if bool_or_default(l, true)? {
                                Visibility::Hidden
                            } else {
                                Visibility::Full
                            };
                            self.last_reg_inst().set_visibility(&ovr_idx, v);
                        }
                        Context::RegIndex(i) => {
                            ovr_idx.set_reg_list(i);
                            let info = reg_inst_array_properties(&mut l)?;
                            match info {
                                Context::Description => {
                                    self.last_hidden = is_hidden(&mut l)?;
                                    if !l.is_empty() {
                                        let hidden = self.last_hidden;
                                        self.last_reg_inst().desc_updt(&ovr_idx, desc(l)?, hidden);
                                    }
                                    context_stack.push((Context::Description, ilvl + 1));
                                }
                                Context::Optional => {
                                    self.last_reg_inst().set_optional(&ovr_idx,  parse_expr(l)?)
                                }
                                Context::Address(addr) => {
                                    self.last_reg_inst().set_addr(&ovr_idx, addr);
                                }
                                Context::Hidden => {
                                    let v = if bool_or_default(l, true)? {
                                        Visibility::Hidden
                                    } else {
                                        Visibility::Full
                                    };
                                    self.last_reg_inst().set_visibility(&ovr_idx, v);
                                }
                                Context::Reserved => {
                                    let v = if bool_or_default(l, true)? {
                                        Visibility::Reserved
                                    } else {
                                        Visibility::Full
                                    };
                                    self.last_reg_inst().set_visibility(&ovr_idx, v);
                                }
                                Context::Disabled => {
                                    self.last_reg_inst()
                                        .set_visibility(&ovr_idx, Visibility::Disabled);
                                }
                                Context::HwAccess => {
                                    return Err(RifError::unsupported(info, l));
                                }
                                Context::Item(n) => {
                                    ovr_idx.set_field_name(n);
                                    self.parse_inst_field(&mut context_stack, ilvl, &ovr_idx, &mut l)?;
                                }
                                _ => {
                                    return Err(RifError::unsupported(info, l));
                                }
                            }
                        }
                        Context::Item(n) => {
                            ovr_idx.set_field_name(n);
                            self.parse_inst_field(&mut context_stack, ilvl, &ovr_idx, &mut l)?;
                        }
                        Context::FieldIndex((n,i)) => {
                            ovr_idx.set_field_list(n,i);
                            self.parse_inst_field(&mut context_stack, ilvl, &ovr_idx, &mut l)?;
                        }
                        _ => {
                            return Err(RifError::unsupported(info, l));
                        }
                    }
                }
                Context::RifInst => {
                    let info = rif_inst_properties(&mut l)?;
                    match info {
                        Context::Description => {
                            self.last_hidden = is_hidden(&mut l)?;
                            if !l.is_empty() {
                                let hidden = self.last_hidden;
                                self.last_rif_inst().description.updt(desc(l)?, hidden);
                            }
                            context_stack.push((Context::Description, ilvl + 1));
                        }
                        Context::Suffix => {
                            self.last_rif_inst().add_suffix(rif_inst_suffix(l)?);
                        }
                        Context::Parameters => context_stack.push((Context::Parameters, ilvl + 1)),
                        Context::Optional => {
                            let expr = parse_expr(l)?;
                            self.last_rif_inst().optional = expr;
                        }
                        _ => {
                            return Err(RifError::unsupported(info, l));
                        }
                    }
                }
                // Unimplemented context
                _ => {
                    return Err(RifError::unsupported(cntxt.0.clone(), l));
                }
            }
            // Potentially finish parsing end of line based on new context
        }
        // Ensure a default Hardware clock is defined
        for rif in self.rifs.values_mut() {
            if rif.hw_clocking.is_empty() {
                rif.hw_clocking.push(ClockingInfo::default());
            }
        }
        Ok(refs)
    }

    // Quick access to currently active object
    // Suppose the function cannot fail since
    // it should only be called in context where a new element was added
    fn last_rifmux(&mut self) -> &mut Rifmux {
        self.rifmux.get_mut(&self.last_obj).expect("No RIFMux")
    }

    fn last_rif_inst(&mut self) -> &mut RifmuxItem {
        self.rifmux
            .get_mut(&self.last_obj)
            .expect("No RIFMux")
            .items
            .last_mut()
            .expect("No RIF Instance")
    }

    fn last_rif(&mut self) -> &mut Rif {
        self.rifs.get_mut(&self.last_obj).expect("No RIF")
    }

    fn last_page_mut(&mut self) -> &mut RifPage {
        self.last_rif().pages.last_mut().expect("No Page")
    }

    fn last_page(&self) -> &RifPage {
        self.rifs.get(&self.last_obj).expect("No RIF").pages.last().expect("No Page")
    }

    fn last_reg_mut(&mut self) -> &mut RegDef {
        self.last_page_mut()
            .registers
            .last_mut()
            .expect("No Registers definition")
            .get_regdef_mut()
            .expect("Not a register definition !")
    }

    fn last_reg(&self) -> &RegDef {
        self.last_page()
            .registers
            .last()
            .expect("No Registers definition")
            .get_regdef()
            .expect("Not a register definition !")
    }

    fn last_reg_inst(&mut self) -> &mut RegInst {
        self.last_page_mut()
            .instances
            .last_mut()
            .expect("No Registers instance")
    }

    fn last_field(&self) -> &Field {
        self.last_reg().fields.last().expect("No Fields")
    }

    fn last_field_mut(&mut self) -> &mut Field {
        self.last_reg_mut().fields.last_mut().expect("No Fields")
    }

    fn parse_inst_field(
        &mut self,
        context_stack: &mut ContextStack,
        ilvl: usize,
        ovr_idx: &OverrideIndex,
        line: &mut &str,
    ) -> Result<(), RifError> {
        let info = reg_inst_field_properties(line)?;
        match info {
            Context::Description => {
                self.last_hidden = is_hidden(line)?;
                if !line.is_empty() {
                    let hidden = self.last_hidden;
                    self.last_reg_inst().desc_updt(ovr_idx, desc(line)?, hidden);
                }
                context_stack.push((Context::Description, ilvl + 1));
            }
            Context::Optional => self.last_reg_inst().set_optional(ovr_idx,  parse_expr(line)?),
            Context::Hidden => {
                let v = if bool_or_default(line, true)? {
                    Visibility::Hidden
                } else {
                    Visibility::Full
                };
                self.last_reg_inst().set_visibility(ovr_idx, v);
            }
            Context::Reserved => {
                let v = if bool_or_default(line, true)? {
                    Visibility::Reserved
                } else {
                    Visibility::Full
                };
                self.last_reg_inst().set_visibility(ovr_idx, v);
            }
            Context::Disabled => {
                self.last_reg_inst().set_visibility(ovr_idx, Visibility::Disabled);
                if let Ok(r) = reset_val(line) {
                    self.last_reg_inst().set_reset(ovr_idx, r);
                }
            }
            Context::HwReset => {
                self.last_reg_inst().set_reset(ovr_idx, reset_val(line)?);
            }
            Context::Limit => {
                self.last_reg_inst().set_limit(ovr_idx, limit_def(line)?);
            }
            Context::Info => context_stack.push((Context::Info, ilvl + 1)),
            _ => {
                return Err(RifError::unsupported(info, line));
            }
        }
        Ok(())
    }

    pub fn get_rif<'a>(&'a self, name: &'a str) -> Option<&'a Rif>{
        get_rif(&self.rifs, name)
    }

    pub fn get_rifmux<'a>(&'a self, name: &'a str) -> Option<&'a Rifmux>{
        get_rif(&self.rifmux, name)
    }

    /// Check that a register is uniquely defined
    pub fn check_reg_uniq(&self, name: &str) -> bool {
        let rif = self.rifs.get(&self.last_obj).expect("No RIF");
        for page in &rif.pages {
            if page.registers.iter()
                    .filter_map(|r| if let RegDefOrIncl::Def(d) = r {Some(d)} else {None})
                    .any(|r| r.name == name) {
                return false;
            }
        }
        true
    }
}


// Remove prefix/suffix rif from a string
pub fn remove_rif(name: &str) -> &str {
    match name {
        s if s.starts_with("rif_") => &s[4..],
        s if s.ends_with("_rif") => &s[..s.len()-4],
        s if s.starts_with("rifmux_") => &s[7..],
        s if s.ends_with("_rifmux") => &s[..s.len()-7],
        s  => s,
    }
}

/// Try to find a rif name in a Hashmap, by checking the name, name_rif and rif_name
pub fn get_rif<'a,T>(dict: &'a HashMap<String,T>, key: &'a str) -> Option<&'a T> {
    if dict.contains_key(key) {
        return dict.get(key);
    }
    for i in 0..4 {
        let k  = match i {
            0 => key.to_owned() + "_rif",
            1 => key.to_owned() + "_rif_mux",
            2 => "rif_".to_owned() + key,
            _ => "rif_mux_".to_owned() + key,
        };
        if dict.contains_key(&k) {
            return dict.get(&k);
        }
    }
    // Everything failed ? return None
    // println!("[get_rif] Unable to find {} in {:?}", key, dict.keys().collect::<Vec<&String>>());
    None
}
