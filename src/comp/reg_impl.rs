use std::collections::BTreeMap;

use crate::{parser::{get_rif, parser_expr::ParamValues}, rifgen::{
    order_dict::{OrderDict, OrderedDictIterV}, Access, ClkEn, Description, EnumKind, ExternalKind, Field, FieldHwKind, FieldSwKind, InterruptDesc, InterruptInfo, Limit, Lock, RegDef, RegDefOrIncl, RegIncludePath, RegPulseKind, ResetVal, Rif
}};

use super::comp_inst::{PartialFieldDict, PartialFieldInfos, RifPageInst, RifRegInst, RifsInfo};

/// Field Implementation
/// Contains all information for the hardware field after compilation
/// This group together the partial definition when split over multiple register definition
#[derive(Clone, Debug, PartialEq)]
pub struct FieldImpl {
    /// Field name
    pub name: String,
    /// Field width
    pub width: u16,
    /// Field array size
    pub array: u16,
    /// Sign-ness
    pub signed: bool,
    /// Reset value
    pub reset: Vec<ResetVal>,
    /// Field description
    pub description: Description,
    /// Optional enumeratino kind
    pub enum_kind: EnumKind,
    /// Field Hardware kind
    pub hw_kind: Vec<FieldHwKind>,
    /// Field software kind
    pub sw_kind: FieldSwKind,
    /// Field hardware access
    pub hw_acc: Access,
    /// Field Clock name: if none select automatically
    pub clk: Option<String>,
    /// Field Clock enable
    pub clk_en: ClkEn,
    /// Field optional clear signal (when high reset field)
    pub clear: Option<String>,
    /// Field optional lock signal (when high field cannot be modified)
    pub lock: Lock,
    /// Optional Interrupt Description
    pub intr_desc: Option<InterruptDesc>,
    /// Values limit
    pub limit: Limit,
    /// High when register value is split on multiple register
    pub is_partial: bool,
    /// Index to the corresponding register control
    pub ctrl_idx: usize,
}

impl FieldImpl {
    fn new(field: &Field, reg_array: u16, ctrl_idx: usize, params: &ParamValues, partials: Option<&PartialFieldInfos>) -> Self {
        let signed = matches!(field.reset.first(), Some(ResetVal::Signed(_)));
        // Handle case of partial array
        let mut array = reg_array.max(1) * field.array.value(params) as u16;
        if reg_array > 0 {
            array += field.partial.1;
        }
        // Handle case of partial field
        let (width, reset) = if field.partial.0.is_some() {
            // By construction the partials should always be Some if the field is partial
            partials.unwrap().merge(&field.name, signed)
        } else {
            (field.width(params) as u16, field.reset.clone())
        };

        // Handle case where only access is a set/clr from software: this implies the equivalent from hardware to be complete
        let mut hw_kind = field.hw_kind.to_owned();
        if let Some(kind) = field.get_auto_hw_kind(params) {
            hw_kind.push(kind);
        }
        FieldImpl {
            name: field.name.clone(),
            width,
            array,
            signed,
            clk: field.clk.clone(),
            reset,
            description: field.description.clone(),
            enum_kind: field.enum_kind.clone(),
            hw_kind,
            sw_kind: field.sw_kind.clone(),
            hw_acc: field.hw_acc,
            clk_en: field.clk_en.clone(),
            clear: field.clear.clone(),
            lock: field.lock.clone(),
            intr_desc: field.intr_desc.clone(),
            limit: field.limit.clone(),
            is_partial: field.partial.0.is_some(),
            ctrl_idx
        }
    }

    /// Get the reset value as an unsigned 128b
    // handle case where field is larger than 128b: change to ibig ?
    pub fn get_reset(&self, idx: usize) -> u128 {
        let idx = if idx >= self.reset.len() {0} else {idx};
        self.reset.get(idx).unwrap().to_u128(self.width as u8)
    }

    /// Flag field which can be set by software
    pub fn is_sw_write(&self) -> bool {
        self.sw_kind!=FieldSwKind::ReadOnly
    }

    /// Flag constant field
    pub fn is_constant(&self) -> bool {
        self.sw_kind==FieldSwKind::ReadOnly && !self.hw_acc.is_writable()
    }

    /// Flag constant field
    pub fn is_counter(&self) -> bool {
        self.hw_kind.iter().any(|k| matches!(k,FieldHwKind::Counter(_)))
    }

    /// Flag field which can be set by hardware
    pub fn is_hw_write(&self) -> bool {
        if self.width > 1 {
            self.hw_kind.iter().any(|k| *k!=FieldHwKind::ReadOnly)
        } else {
            self.hw_kind.iter().any(|k| k.has_write_mod() || k.is_counter() || k.is_interrupt())
        }
    }

    /// Flag field which has a Hardware Write Enable
    pub fn has_hw_we(&self) -> bool {
        self.hw_kind.iter().any(|x| x.has_we())
    }

    /// Flag field which can be set by hardware by value
    pub fn has_hw_value(&self) -> bool {
        if self.hw_kind.is_empty() && self.hw_acc==Access::WO {
            true
        } else if self.width > 1 {
            self.hw_kind.iter().any(|k| !matches!(k,FieldHwKind::ReadOnly | FieldHwKind::Counter(_)))
        } else {
            self.hw_kind.iter().any(|k| k.has_we() || k.is_interrupt())
        }
    }

    /// Flag field which has a Hardware Write Enable
    pub fn has_write_mod(&self) -> bool {
        self.hw_kind.iter().any(|x| x.has_write_mod())
    }

    /// Flag field which requires a local flop
    pub fn is_local(&self) -> bool {
        self.has_write_mod() && !self.hw_acc.is_readable()
    }

}

#[allow(dead_code)]
pub fn get_attr_name<'a>(hw_kind: &'a FieldHwKind, regname: &str) -> (Option<&'a str>,Option<&'a str>) {
    let (info, suffix) = match hw_kind {
        FieldHwKind::Set(info)      => (info,"_hwset"),
        FieldHwKind::Toggle(info)   => (info,"_hwtgl"),
        FieldHwKind::Clear(info)    => (info,"_hwclr"),
        FieldHwKind::WriteEn(info)  => (info,"_we"),
        FieldHwKind::WriteEnL(info) => (info,"_wel"),
        _ => (&None,"")
    };
    if suffix.is_empty() {
        (None,None)
    } else if let Some(path) = &info {
        let mut parts = path.split('.');
        match parts.next() {
            Some(s) if s==regname || s=="this" => {
                (parts.next(),None)
            },
            Some("") => (None, Some(suffix)),
            _ => (None,None),
        }
    } else {
        (Some(suffix),None)
    }
}

///
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegPortKind {
    /// Input port only (i.e. driven by hardware)
    In,
    /// Output port only (i.e. driven by software)
    Out,
    /// Input and output port
    InOut,
    /// No ports, internal signal only
    None
}

impl RegPortKind {
    pub fn updt(&mut self, val: RegPortKind) {
        match self {
            RegPortKind::None => *self = val,
            RegPortKind::In => if val==RegPortKind::Out ||  val==RegPortKind::InOut {*self = RegPortKind::InOut},
            RegPortKind::Out => if val==RegPortKind::In ||  val==RegPortKind::InOut {*self = RegPortKind::InOut},
            RegPortKind::InOut => {},
        }
    }

    pub fn from_field(field: &Field) -> Self {
        match field.hw_acc {
            Access::NA => RegPortKind::None,
            Access::WO => RegPortKind::In,
            Access::RW => RegPortKind::InOut,
            Access::RO => {
                if field.hw_kind.is_empty() && field.get_local_lock().is_none() {
                    RegPortKind::Out
                } else {
                    RegPortKind::InOut
                }
            },
        }
    }

    pub fn from_acc(hw_acc: &Access) -> Self {
        match hw_acc {
            Access::NA => RegPortKind::None,
            Access::WO => RegPortKind::In,
            Access::RW => RegPortKind::InOut,
            Access::RO => RegPortKind::Out,
        }
    }

    pub fn from_reg(reg: &RegDef) -> Self {
        if reg.external!=ExternalKind::None {RegPortKind::InOut}
        else if !reg.pulse.is_empty() {RegPortKind::Out}
        else {RegPortKind::None}
    }

    pub fn is_in(&self) -> bool {
        self==&RegPortKind::In || self==&RegPortKind::InOut
    }

    pub fn is_out(&self) -> bool {
        self==&RegPortKind::Out || self==&RegPortKind::InOut
    }
}

/// Register Hardware control: pulses and external kind
#[derive(Clone, Debug, PartialEq)]
pub struct RegHwCtrl {
    pub name: String,
    pub pulse: Vec<RegPulseKind>,
    pub external: ExternalKind
}

impl RegHwCtrl {
    pub fn new(name: String, pulse: Vec<RegPulseKind>, external: ExternalKind) -> Self {
        RegHwCtrl { name, pulse, external }
    }

    pub fn is_external(&self) -> bool {
        self.external.is_rw()
        // self.external!=ExternalKind::None
    }
}

#[derive(Clone, Debug)]
pub struct RegImplDict(OrderDict<String, RegImpl>);

impl RegImplDict {

    pub fn build(src: &Rif, rifs: &RifsInfo) -> Result<RegImplDict, String> {
        let nb_reg = src.pages.first().map(|p| p.registers.len()).unwrap_or(0);
        let mut regs = RegImplDict::with_capacity(nb_reg);
        for page in src.pages.iter() {
            if page.external {
                continue;
            }
            for reg_def in page.registers.iter() {
                regs.add_def(reg_def, &page.clk_en, rifs)?;
            }
        }
        Ok(regs)
    }

    pub fn with_capacity(n: usize) -> Self {
        RegImplDict(OrderDict::with_capacity(n))
    }

    pub fn get_mut(&mut self, key: &String) -> Option<&mut RegImpl> {
        self.0.get_mut(key)
    }

    pub fn get(&self, key: &String) -> Option<&RegImpl> {
        self.0.get(key)
    }

    pub fn insert(&mut self, key: String, value: RegImpl) {
        self.0.insert(key, value)
    }

    pub fn values(&self) -> OrderedDictIterV<RegImpl>{
        self.0.values()
    }

    pub fn add_def(&mut self, def: &RegDefOrIncl, clk_en: &ClkEn, rifs: &RifsInfo) -> Result<(), String> {
        match def {
            RegDefOrIncl::Include(inc) => {
                let path = RegIncludePath::new(inc)?;
                let Some(rif) = get_rif(rifs.rifs, path.rif) else {
                    return Err(format!("Unable to find {} in RIF definitions ({:?})", path.rif , rifs.rifs.keys()));
                };
                let Some(inc_page) = rif.pages.iter().find(|x| x.name == path.page) else {
                    return Err(format!("Unable to find page {} in {})", path.page, path.rif));
                };
                // Scan the page for matching registers
                for reg_def in inc_page.registers.iter() {
                    if path.reg=="*" || path.reg==reg_def.get_name() {
                        self.add_def(reg_def, clk_en, rifs)?;
                        if let Some(reg_impl) = self.0.last_mut() {
                            reg_impl.pkg = Some(rif.name.to_owned());
                            // reg_impl.pkg = Some(path.rif.to_string());
                            // println!("Adding Include {}.{}.{}", path.rif, path.page, reg_impl.name);
                        }
                    }
                }
            },
            RegDefOrIncl::Def(reg) => {
                // Skip optional register
                if reg.ignored(&rifs.params) {
                    return Ok(());
                }
                // If register group was already seen, merge fields
                if let Some(reg_impl) = self.get_mut(&reg.group.name) {
                    reg_impl.merge_with(reg, &rifs.params, &rifs.partials)?;
                }
                else {
                    let mut reg_impl = RegImpl::new(reg, &rifs.params, &rifs.partials);
                    // Inherit clock from page if default
                    if reg_impl.clk_en.is_default() {
                        reg_impl.clk_en = clk_en.to_owned();
                    }
                    self.insert(reg_impl.name.clone(), reg_impl);
                }
            }
        }
        Ok(())
    }
}

/// Register Implementation
/// Group all fields belonging to the same group
#[derive(Clone, Debug, PartialEq)]
pub struct RegImpl {
    /// Register name
    pub name: String,
    /// Register Description
    pub description: Description,
    /// Register special control such as read/write pulse
    pub regs_ctrl: Vec<RegHwCtrl>,
    /// Fields inside the register
    pub fields: Vec<FieldImpl>,
    /// Interrupt configuration: if none register is not an interrupt
    pub interrupt: Vec<InterruptInfo>,
    /// Register Clock name: if none select clock automatically
    pub clk: Option<String>,
    /// Register reset name: if none select reset automatically
    pub rst: Option<String>,
    /// Register clock enable name: if none select clock enable automatically
    pub clk_en: ClkEn,
    /// Register clear name: if none, register has no clear
    pub clear: Option<String>,
    /// Indicates the port direction for the register
    pub port: RegPortKind,
    /// Optional package name where the struct is already defined
    pub pkg: Option<String>,
}


impl RegImpl {

    /// Create a register hardware implementation based on a register definition
    fn new(reg: &RegDef, params: &ParamValues, partials: &PartialFieldDict) -> Self {
        let mut fields = Vec::with_capacity(reg.fields.len());
        let mut port = RegPortKind::from_reg(reg);
        let array = reg.array.value(params) as u16;
        let mut sw_access = Access::NA;
        // Copy all fields
        for f in reg.fields.iter() {
            port.updt(RegPortKind::from_field(f));
            sw_access.updt((&f.sw_kind).into());
            fields.push(FieldImpl::new(f, array, 0, params, partials.get(reg.get_group_name())));
        }
        RegImpl {
            name: reg.get_group_name().to_owned(),
            description: reg.description.clone(),
            fields, port,
            interrupt: reg.interrupt.clone(),
            clk: reg.clk.clone(),
            rst: reg.rst.clone(),
            clk_en: reg.clk_en.clone(),
            clear: reg.clear.clone(),
            pkg: reg.group.pkg.clone(),
            regs_ctrl: vec![RegHwCtrl::new(reg.name.clone(), reg.pulse.clone(), reg.external.with_access(&sw_access))],
        }
    }

    /// Merge a register definition in an already existing register implementation
    // Check to do:
    // - Clock, reset, clock enable, clear, external must be the same ?
    // - Interrupt setting must the same
    // - Save partial info to check no overlap or missing
    pub fn merge_with(&mut self, reg: &RegDef, params: &ParamValues, partials: &PartialFieldDict) -> Result<(),String>{
        let array = reg.array.value(params) as u16;
        // println!("Merging {} in {} : clk_en = {:?} | group clock_enable = {:?}", reg.name, reg.group.name, reg.clk_en, self.clk_en);
        self.port.updt(RegPortKind::from_reg(reg));
        for f in reg.fields.iter() {
            self.port.updt(RegPortKind::from_field(f));
            let clk_en = if !f.clk_en.is_default() {&f.clk_en} else {&reg.clk_en};
            // Search field vec in reverse since it is most likely to be the most recent one
            if let Some(ref mut field_impl) = self.fields.iter_mut().rev().find(|e| e.name==f.name) {
                // Check for clk_en/rst: either defined once, or same for all partial definition
                if !clk_en.is_default() {
                    if field_impl.clk_en.is_default() {
                        field_impl.clk_en = clk_en.to_owned();
                    }
                    else if clk_en!=&field_impl.clk_en {
                        return Err(format!("Register group {} has multiple clocks !", reg.get_group_name()))
                    }
                }
                // Partial Array
                if f.partial.1 > 0 && field_impl.array > 0 {
                    if f.partial.0.is_some() {
                        return Err(format!("Field {}.{} : arrays of partial field is not supported !", reg.name, f.name))
                    }
                    // Check contiguous partial array in increasing order
                    if field_impl.array != f.partial.1 {
                        return Err(format!("Field {}.{} : Non contiguous partial field array. Expecting {} found {}", reg.name, f.name, field_impl.array, f.partial.1));
                    }
                    field_impl.array += f.array.value(params) as u16;
                    // TODO: might need to check dimensions (or maybe shjould be done at parsing level)
                    for r in f.reset.iter() {
                        field_impl.reset.push(r.clone());
                    }
                } else if f.partial.0.is_some() {
                    for kind in f.hw_kind.iter() {
                        if !field_impl.hw_kind.iter().any(|k| k==kind) {
                            // println!("Field {} kind updated on {} : {:?}", field_impl.name, f.name, kind);
                            field_impl.hw_kind.push(kind.to_owned());
                        }
                    }
                } else {
                    return Err(format!("Field {}.{} already defined in this register group. Missing partial definition ?", reg.name, f.name));
                }
            } else {
                let mut field = FieldImpl::new(f, array, self.regs_ctrl.len(), params, partials.get(reg.get_group_name()));
                if !clk_en.is_default() {
                    field.clk_en = clk_en.to_owned()
                }
                self.fields.push(field);
            }
        }
        self.regs_ctrl.push(RegHwCtrl::new(reg.name.clone(), reg.pulse.clone(), reg.external));
        Ok(())
    }

    /// Create a register hardware implementation
    pub fn build(
        rif: &Rif,
        group_name: &str,
        rifs: &RifsInfo,
    ) -> Result<Self,String> {
        let mut reg_impl : Option<Self> = None;
        for p in &rif.pages {
            for r in &p.registers {
                match r {
                    RegDefOrIncl::Include(inc) => {
                        let path = RegIncludePath::new(inc).unwrap();
                        let Some(rif) = get_rif(rifs.rifs, path.rif) else {
                            panic!("Unable to find {} in RIF definitions ({:?})", path.rif , rifs.rifs.keys());
                        };
                        let Some(_inc_page) = rif.pages.iter().find(|x| x.name == path.page) else {
                            panic!("Unable to find page {} in {})", path.page, path.rif);
                        };
                        // TODO !!!
                    }
                    RegDefOrIncl::Def(d) => if d.group.name == group_name {
                        if let Some(ref mut reg) = reg_impl {
                            reg.merge_with(d, &rifs.params, &rifs.partials)?;
                        } else {
                            reg_impl = Some(RegImpl::new(d, &rifs.params, &rifs.partials));
                        }
                    }
                }
            }
        }
        reg_impl.ok_or(format!("Register group {} not found !", group_name))
    }

    /// Get a reference to the field implementation definition by name
    pub fn get_field(&self, name: &str) -> Result<&FieldImpl,String> {
        self.fields.iter().find(|f| f.name==*name)
            .ok_or(format!("Field {name} should be defined in register implementation of {}", self.name))
    }

    /// Flag when a register is an interrupt register
    pub fn is_interrupt(&self) -> bool {
        !self.interrupt.is_empty()
    }

    /// Indicate the register need a software structure which is not visible from the hardware
    pub fn is_local(&self) -> bool {
        if !self.interrupt.is_empty() {
            return false;
        }
        let nb_fields = self.fields.iter().filter(|f| !f.is_local()).count();
        // If all fields are local, there is no sw structure -> every field is stored in local register
        if nb_fields == 0 {
            return false;
        }
        // println!("Reg {} : port={:?} | Fields: constant?{}, sw_write?{}", self.name, self.port, self.fields.iter().any(|f| f.is_constant()), self.fields.iter().any(|f| f.is_sw_write()));
        match self.port {
            RegPortKind::None => true,
            RegPortKind::Out |
            RegPortKind::InOut => false,
            RegPortKind::In =>
                self.fields.iter().filter(|f| !f.is_local()).any(|f|
                    f.is_constant() ||
                    f.is_sw_write() ||
                    f.is_counter() ||
                    f.has_hw_we() && f.hw_acc!=Access::NA
                ),
        }
    }

    /// Indicate if the register group contains multiple register control signals
    pub fn is_multi_pulse(&self) -> bool {
        self.regs_ctrl.iter().map(|c| if c.pulse.is_empty() {0} else {1}).sum::<usize>() > 1
    }

    /// Retrieve interrupt information
    pub fn intr_info(&self, reg: &RifRegInst) -> Result<&InterruptInfo, String> {
        let intr_name = reg.intr_info.1.strip_prefix('_').unwrap_or(&reg.intr_info.1);
        self.interrupt.iter()
            .find(|intr| intr.name==intr_name)
            .ok_or(
                format!("Interrupt {}{} ({:?}) instance implementation should have interrupt info : {:?}",
                    reg.reg_name,
                    reg.intr_info.1,
                    reg.intr_info.0,
                    self.interrupt.iter().map(|i| &i.name).collect::<Vec<_>>())
            )
    }
}

#[derive(Clone, Debug)]
pub struct MissingFieldInfo {
    /// Width of the field
    pub width: u16,
    /// Signed-ness
    pub signed: bool,
    /// Signed-ness
    pub reset: u128,
}

impl MissingFieldInfo {
    fn from(value: &FieldImpl, idx: usize) -> Self {
        MissingFieldInfo {
            width: value.width,
            signed: value.signed,
            reset: value.get_reset(idx),
        }
    }
}

#[derive(Clone, Debug)]
pub struct HwRegInst {
    /// Group type of the register
    pub group: String,
    /// Dimension of the register: 0 if not an array of registers
    pub dim: u16,
    /// Port direction
    pub port: RegPortKind,
    /// Register instance derived (interrupt en/mask/pending)
    pub intr_derived: bool,
    /// Missing fields
    pub missing_fields: BTreeMap<String, MissingFieldInfo>,
}

impl HwRegInst {
    pub fn new(group: String, array_size: u16, port: RegPortKind, intr_derived: bool, missing_fields: BTreeMap<String, MissingFieldInfo>) -> Self {
        HwRegInst {group, dim: array_size, port, intr_derived, missing_fields}
    }
}

pub type HwRegs = OrderDict<String, HwRegInst>;

impl HwRegs {
    pub fn build(
        pages: &[RifPageInst],
        defs: &RegImplDict,
        rifs: &RifsInfo,
    ) -> Result<HwRegs, String> {
        let mut hw_regs = HwRegs::new();
        for page in pages {
            for reg in &page.regs {
                let group_name = format!("{}{}", reg.group_name, reg.intr_info.0.get_suffix());
                // let group_name = reg.group_name.to_owned();
                // println!("{:?} ({:?}) : {:?}", reg.reg_name, group_name, reg.intr_info);
                // If instance already exists remove every field from the register instance
                if let Some(hw_reg) = hw_regs.get_mut(&group_name) {
                    for fr in reg.fields.iter() {
                        // let name = if let Some(idx) = fr.idx {format!("{}[{idx}]", fr.name)} else {fr.name.to_owned()};
                        let name = fr.name();
                        hw_reg.missing_fields.remove(&name);
                    }
                }
                // If instance does not exist insert a new hardware register instance
                else {
                    // Register implementation should always exists (except if it was badly constructed)
                    let Some(reg_impl) = defs.get(&reg.group_type) else {
                        return Err(format!("Register group {} should be defined in the hardware registers !", group_name))
                    };
                    // println!("[HwRegs] reg {} : port = {:?} | {:?}", group_name, reg_impl.port, reg_impl.fields.iter().map(|f| (&f.name,&f.hw_acc)).collect::<Vec<(&String,&Access)>>());
                    let dim = reg.array.dim();
                    let ext_reg_impl;
                    let fields = if let Some(pkg) = &reg_impl.pkg {
                        // Register defined in another RIF: need to build its implementation
                        let Some(rif) = get_rif(rifs.rifs, pkg) else {
                            return Err(format!("Unable to find {} in RIF definitions ({:?})", pkg , rifs.rifs.keys()))
                        };
                        ext_reg_impl = RegImpl::build(rif, &reg.group_type, rifs).unwrap();
                        &ext_reg_impl.fields
                    } else {
                        &reg_impl.fields
                    };
                    // Init the missing fields with all field from the register implementation not defined in the current register instance
                    let mut missing = BTreeMap::new();
                    if !reg.intr_info.0.is_derived() {
                        for fi in fields.iter()
                            .filter(
                                |fi| (fi.is_sw_write() || fi.has_write_mod() || reg.is_intr())
                                    && !reg.fields.iter().any(|fr| fr.name == fi.name) ) {
                            if fi.array > 0 {
                                // TODO handle partially implemented array: might need to remove the condition check on name inside the filter
                                for i in 0..fi.array {
                                    missing.insert(format!("{}[{i}]", fi.name), MissingFieldInfo::from(fi,i as usize));
                                }
                            } else {
                                missing.insert(fi.name.to_owned(), MissingFieldInfo::from(fi,0));
                            }
                        }
                    }
                    let mut port = if reg.intr_info.0.is_derived() {RegPortKind::None} else {reg_impl.port};
                    port.updt(RegPortKind::from_acc(&reg.hw_access));
                    // println!("{:?} ({:?}) : {:?} ", reg.reg_name, group_name, port);
                    hw_regs.insert(
                        group_name.clone(),
                        HwRegInst::new(reg.group_type.clone(), dim, port, reg.intr_info.0.is_derived(), missing)
                    );
                }
            }
        }
        Ok(hw_regs)
    }
}