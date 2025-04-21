use crate::{
    generator::casing::{Casing, ToCasing},
    rifgen::{ClkEn, FieldHwKind, Interface, ResetVal, Rif, SuffixInfo}
};

use super::{
    comp_inst::{RifFieldInst, RifPageInst},
    reg_impl::{FieldImpl, HwRegs, MissingFieldInfo, RegImplDict}
};

/// Signal type: either a bit vector (signed or unsigned) or a custom type
#[derive(Clone, Debug, PartialEq)]
pub enum SignalKind {
    /// Single bit or unsigned bus
    Unsigned(u16),
    /// Signed bus
    Signed(u16),
    /// Custom type (package+type)
    Custom((Option<String>,String)),
    /// Intrinsic integer type
    Integer,
    /// Address bus (size if determined by context)
    Address,
    /// Data bus (size if determined by context)
    Data,
}

impl SignalKind {
    pub fn new(width: u16, signed: bool) -> Self {
        if signed {
            SignalKind::Signed(width)
        } else {
            SignalKind::Unsigned(width)
        }
    }

    pub fn from_field(f: &FieldImpl, base_scope: &str, reg_type: &str) -> Self {
        if let Some((scope,name)) = f.enum_kind.get_type(base_scope, reg_type, &f.name) {
            SignalKind::Custom((Some(scope), name))
        } else {
            let w = if f.sw_kind.is_password() {2} else {f.width};
            if f.signed {
                SignalKind::Signed(w)
            } else {
                SignalKind::Unsigned(w)
            }
        }
    }

    pub fn set_width(&mut self, width: u16) {
        match self {
            SignalKind::Unsigned(w) => *w = width,
            SignalKind::Signed(w) => *w = width,
            _ => {}
        }
    }

    pub fn custom_name(&self) -> &str {
        match self {
            SignalKind::Custom((_,n)) => n,
            _ => ""
        }
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, SignalKind::Custom(_))
    }
}

/// Signal declaration info: contains both name and type
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SignalDef {
    /// Signal name
    pub name: String,
    /// Signal kind (unsigned/signed/custom type)
    pub kind: SignalKind,
    /// Array dimension (0 means not an array)
    pub dim: u16,
}

impl SignalDef {
    pub fn new(name: String, kind: SignalKind, dim: u16) -> Self {
        SignalDef { name, kind, dim}
    }

    pub fn new_bit(name: String) -> Self {
        SignalDef { name, kind: SignalKind::Unsigned(1), dim: 0}
    }

    pub fn new_bus(name: String, width: u16, signed: bool) -> Self {
        SignalDef { name, kind: SignalKind::new(width, signed), dim: 0}
    }

    pub fn new_int(name: String) -> Self {
        SignalDef { name, kind: SignalKind::Integer, dim: 0}
    }

    /// Define a signal with User-define type
    pub fn new_ud(name: String, scope: String, type_name: String, dim: u16) -> Self {
        SignalDef { name, kind: SignalKind::Custom((Some(scope), type_name)), dim}
    }
}


/// Signal declaration info: contains both name and type
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SignalDecl {
    /// Signal definition (name, kind, dim)
    pub def: SignalDef,
    /// Signal description
    pub desc: String,
}

impl SignalDecl {
    pub fn new(def: SignalDef, desc: String) -> Self{
        SignalDecl {def, desc}
    }

    pub fn new_bit(name: String, desc: String) -> Self{
        SignalDecl {def: SignalDef::new_bit(name), desc}
    }

    pub fn new_bus(name: String, width: u16, signed: bool, desc: String) -> Self{
        SignalDecl {def: SignalDef::new_bus(name, width, signed), desc}
    }

    pub fn from_hw_kind(kind: &FieldHwKind, regname: &str, fieldname: &str) -> Option<Self> {
        let name = if let Some(path) = kind.get_signal() {
            let mut parts = path.split('.');
            match (parts.next(),parts.next()) {
                (Some(f),None) => f.to_owned(),
                (Some(r),Some(f)) if r == regname || r == "this" || r == "self" => f.to_owned(),
                _ => return None
            }
        } else {
            format!("{}{}", fieldname, kind.get_suffix())
        };
        Some(SignalDecl {
            def: SignalDef::new_bit(name),
            desc: kind.get_comment(fieldname)
        })
    }

    pub fn name(&self) -> &str {
        &self.def.name
    }
}

impl From<SignalDef> for SignalDecl {
    fn from(value: SignalDef) -> Self {
        SignalDecl {def: value, desc: "".to_owned()}
    }
}

impl From<&str> for SignalDecl {
    fn from(name: &str) -> Self {
        SignalDecl::new_bit(name.to_owned(), "".to_owned())
    }
}

impl From<(&str, u16)> for SignalDecl {
    fn from(tuple: (&str, u16)) -> Self {
        SignalDecl::new_bus(tuple.0.to_owned(), tuple.1, false, "".to_owned())
    }
}

pub struct SignalInfo {
    pub name: ExprId,
    pub reset: LogicExpr,
    pub value: LogicExpr,
    pub enable: Option<LogicExpr>,
    pub clear: Option<LogicExpr>,
}

impl SignalInfo {
    pub fn new(name: ExprId, reset: LogicExpr, value: LogicExpr) -> SignalInfo {
        SignalInfo {
            name: name.to_owned(),
            reset, value,
            enable: None,
            clear: None,
        }
    }

    pub fn new_with_en( name: ExprId, reset: LogicExpr, value: LogicExpr, enable: Option<LogicExpr>) -> SignalInfo {
        SignalInfo {
            name: name.to_owned(),
            reset,
            value,
            enable,
            clear: None,
        }
    }

    #[allow(dead_code)]
    pub fn new_with_en_clr(name: ExprId, reset: LogicExpr, value: LogicExpr, enable: Option<LogicExpr>, clear: Option<LogicExpr>) -> SignalInfo {
        SignalInfo {
            name: name.to_owned(),
            reset,
            value,
            enable,
            clear}
    }
}

/// Hardware port information: name, width, direction, description
#[derive(Clone, Debug)]
pub struct PortInfo {
    pub def: SignalDef,
    pub dir: PortDir,
    pub desc: String,
}

impl PortInfo {
    pub fn new_in(name: String, desc: String) -> Self {
        PortInfo {
            def: SignalDef::new_bit(name),
            dir: PortDir::In,
            desc,
        }
    }

    pub fn new_out(name: String, desc: String) -> Self {
        PortInfo {
            def: SignalDef::new_bit(name),
            dir: PortDir::Out,
            desc,
        }
    }

    pub fn new_intf(name: String, if_name: String, modport: String, desc: String) -> Self {
        PortInfo {
            def: SignalDef::new(name, SignalKind::Custom((None,if_name)), 0),
            dir: PortDir::Modport(modport),
            desc,
        }
    }

    pub fn new(name: String, kind: SignalKind, dir: PortDir, dim: u16, desc: String) -> Self {
        PortInfo {
            def: SignalDef::new(name, kind, dim),
            dir,
            desc,
        }
    }

    pub fn is_intf(&self) -> bool {
        matches!(self.dir, PortDir::Modport(_))
    }

    pub fn name(&self) -> &str {
        &self.def.name
    }

    pub fn dim(&self) -> u16 {
        self.def.dim
    }

    pub fn kind(&self) -> &SignalKind {
        &self.def.kind
    }

    /// Return the port width
    /// Width is 0 for custom type since size cannot be known
    pub fn width(&self, addr_w: u8, data_w: u8) -> u16 {
        match self.def.kind {
            SignalKind::Unsigned(w) => w,
            SignalKind::Signed(w)   => w,
            SignalKind::Integer     => 32, // Both SV & VHDL use 32bits for integer type
            SignalKind::Address     => addr_w as u16,
            SignalKind::Data        => data_w as u16,
            SignalKind::Custom(_)   => 0,
        }
    }
}

/// Port Direction: input/Output/ModPort
#[derive(Clone, Debug, PartialEq)]
pub enum PortDir {
    /// Input port
    In,
    /// Output port
    Out,
    /// Defines the names of the two possible modport
    Modport(String)
}

impl PortDir {
    /// True for intput port
    pub fn is_in(&self) -> bool {
        self == &PortDir::In
    }
    /// True for output port
    pub fn is_out(&self) -> bool {
        self == &PortDir::Out
    }
    /// True when direction is a modport (interface)
    pub fn is_modport(&self) -> bool {
        matches!(self, PortDir::Modport(_))
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct PortList {
    pub clocks : Vec<PortInfo>,
    pub resets : Vec<PortInfo>,
    pub clk_ens: Vec<PortInfo>,
    pub ctrls  : Vec<PortInfo>,
    pub regs   : Vec<PortInfo>,
    pub irqs   : Vec<PortInfo>,
    pub pages  : Vec<PortInfo>,
}

#[allow(dead_code)]
impl PortList {
    pub fn new(rif: &Rif, pages_inst: &[RifPageInst], regs_impls: &RegImplDict, hw_regs: &HwRegs, suffix: &Option<SuffixInfo>) -> Self {
        let mut clocks : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut resets : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut clk_ens : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut ctrls  : Vec<PortInfo> = Vec::with_capacity(2);
        let mut regs   : Vec<PortInfo> = Vec::with_capacity(hw_regs.len());
        let mut irqs   : Vec<PortInfo> = Vec::with_capacity(2) ;
        // Software clocking
        clocks.push(PortInfo::new_in(rif.sw_clocking.clk.to_owned(), "Software clock".to_owned()));
        resets.push(PortInfo::new_in(
            rif.sw_clocking.rst.name.to_owned(),
            format!("Software {}", rif.sw_clocking.rst.desc())
        ));
        if !rif.sw_clocking.en.is_empty() {
            clk_ens.push(PortInfo::new_in(rif.sw_clocking.en.to_owned(), "Software clock enable".to_owned()))
        }
        if !rif.sw_clocking.clear.is_empty() {
            ctrls.push(PortInfo::new_in(
                rif.sw_clocking.clear.to_owned(),
                "Software clear".to_owned()
            ));
        }
        // Hardware Clocking
        for hw_clk in rif.hw_clocking.iter() {
            if !clocks.iter().any(|p| p.name()==hw_clk.clk) {
                clocks.push(PortInfo::new_in(hw_clk.clk.to_owned(), "Hardware clock".to_owned()));
            }
            if !resets.iter().any(|p| p.name()==hw_clk.rst.name) {
                resets.push(PortInfo::new_in(
                    hw_clk.rst.name.to_owned(),
                    format!("Hardware {}", rif.sw_clocking.rst.desc())));
            }
            if !hw_clk.en.is_empty() && !clk_ens.iter().any(|p| p.name()==hw_clk.en) {
                clk_ens.push(PortInfo::new_in(hw_clk.en.to_owned(), "Hardware clock enable".to_owned()));
            }
            if !hw_clk.clear.is_empty() && !ctrls.iter().any(|p| p.name()==hw_clk.clear) {
                ctrls.push(PortInfo::new_in(
                    hw_clk.clear.to_owned(),
                    "Hardware clear".to_owned()
                ));
            }
        }
        // Collect Clock enable and controls signals from register implementation
        for r in regs_impls.values() {
            if let ClkEn::Signal(en) = &r.clk_en {
                if !clk_ens.iter().any(|p| p.name()==en) {
                    clk_ens.push(PortInfo::new_in(en.to_owned(), "Clock enable".to_owned()));
                }
            }
            for f in r.fields.iter() {
                if let ClkEn::Signal(en) = &f.clk_en {
                    if !clk_ens.iter().any(|p| p.name()==en) {
                        clk_ens.push(PortInfo::new_in(en.to_owned(), "Clock enable".to_owned()));
                    }
                }
                if let Some(lock) = f.lock.port_name() {
                    if !ctrls.iter().any(|p| p.name()==lock) {
                        ctrls.push(PortInfo::new_in(lock.to_owned(), "Lock signal".to_owned()));
                    }
                }
                for k in f.hw_kind.iter() {
                    if let Some(sig) = k.get_signal() {
                        if let Some(sig) = sig.strip_prefix('.') {
                            if !ctrls.iter().any(|p| p.name()==sig) {
                                ctrls.push(PortInfo::new_in(sig.to_owned(), "Control signal".to_owned()));
                            }
                        }
                    }
                }
            }
        }
        //
        let pages : Vec<PortInfo> = pages_inst.iter()
            .filter(|p| p.is_external())
            .map(|p| PortInfo::new_intf(
                p.name.to_owned(),
                "rif_if".to_owned(), "rif".to_owned(),
                format!("Interface to access register from page {}", p.name)))
            .collect();
        let rif_pkg_name = if let Some(suffix) = suffix {
            if !suffix.pkg  {
                rif.name.to_owned()
            } else if suffix.alt_pos && rif.name.ends_with("_rif") {
                format!("{}_{}_rif", &rif.name[..(rif.name.len()-4)], suffix.name)
            } else {
                format!("{}_{}", rif.name, suffix.name)
            }
        } else {
            rif.name.to_owned()
        };
        let rif_pkg_name = rif_pkg_name.to_casing(Casing::Snake);
        for (group_name, hw_reg) in hw_regs.items() {
            let hw_reg_def = regs_impls.get(&hw_reg.group).unwrap();
            let pkg_base = if let Some(pkg) = &hw_reg_def.pkg {pkg} else {&rif_pkg_name};
            let pkg_name = format!("{}_pkg", pkg_base.to_casing(Casing::Snake));
            let group_type = hw_reg.group.to_casing(Casing::Snake);
            let desc = hw_reg_def.description.get_short();
            if hw_reg.port.is_in() {
                let port = PortInfo::new(
                    group_name.to_owned(),
                    SignalKind::Custom((Some(pkg_name.clone()), format!("t_{group_type}_hw"))),
                    PortDir::In,
                    hw_reg.dim,
                    desc.to_owned(),
                );
                regs.push(port);
            }
            if hw_reg.port.is_out() {
                let suffix = if hw_reg.intr_derived {"hw"} else {"sw"};
                let port = PortInfo::new(
                    format!("rif_{group_name}"),
                    SignalKind::Custom((Some(pkg_name), format!("t_{group_type}_{suffix}"))),
                    PortDir::Out,
                    hw_reg.dim,
                    desc.to_owned(),
                );
                regs.push(port);
            }
            if !hw_reg_def.interrupt.is_empty() && !hw_reg.intr_derived {
                let port = PortInfo::new_out(
                    format!("rif_{group_name}_irq"),
                    format!("High when one interrupt field of {group_name} is asserted"));
                irqs.push(port);
                for info in hw_reg_def.interrupt.iter().skip(1) {
                    let port = PortInfo::new_out(
                        format!("rif_{group_name}_{}_irq", info.name),
                        format!("High when one interrupt field of {group_name}_{} is asserted", info.name));
                    irqs.push(port);
                }
            }
        }
        PortList {
            clocks,
            resets,
            clk_ens,
            ctrls,
            regs ,
            irqs ,
            pages,
        }
    }
}

#[derive(Clone, Debug, Default)]
/// Define ports for a RIF interface
pub struct RifIntfPorts (Vec<PortInfo>);

impl RifIntfPorts {
    pub fn new(intf: &Interface, use_intf: bool) -> Self {
        let ports =
        match intf {
            Interface::Default => {
                if use_intf {
                    vec![
                        PortInfo::new_intf(
                            "if_rif".to_owned(),
                            "rif_if".to_owned(), "rif".to_owned(),
                            "SW register interface".to_owned())
                    ]
                } else {
                    vec![
                        PortInfo::new(    "reg_addr           ".to_owned(), SignalKind::Address, PortDir::In, 0, "Register address".to_owned()),
                        PortInfo::new_in( "reg_en             ".to_owned(), "Register enable".to_owned()),
                        PortInfo::new_in( "reg_rd_wrn         ".to_owned(), "Register write/not read".to_owned()),
                        PortInfo::new(    "reg_wr_data        ".to_owned(), SignalKind::Data, PortDir::In , 0, "Register write data".to_owned()),
                        PortInfo::new(    "reg_rd_data        ".to_owned(), SignalKind::Data, PortDir::Out, 0, "Register read data".to_owned()),
                        PortInfo::new_out("reg_done           ".to_owned(), "Register ready".to_owned()),
                        PortInfo::new_out("reg_done_next      ".to_owned(), "Register ready next".to_owned()),
                        PortInfo::new_out("reg_err_addr       ".to_owned(), "Register address error".to_owned()),
                        PortInfo::new_out("reg_err_addr_next  ".to_owned(), "Register address error (combinatorial)".to_owned()),
                        PortInfo::new_out("reg_err_access     ".to_owned(), "Register access error".to_owned()),
                        PortInfo::new_out("reg_err_access_next".to_owned(), "Register access error (combinatorial)".to_owned()),
                    ]
                }
            },
            Interface::Apb => vec![
                PortInfo::new("paddr".to_owned(), SignalKind::Address, PortDir::In, 0, "APB Address".to_owned()),
                PortInfo::new_in("psel".to_owned(), "APB Select".to_owned()),
                PortInfo::new_in("penable".to_owned(), "APB Enable".to_owned()),
                PortInfo::new_in("pwrite".to_owned(), "APB Write".to_owned()),
                PortInfo::new("pwdata".to_owned(), SignalKind::Data, PortDir::In, 0, "APB Write Data".to_owned()),
                PortInfo::new("prdata".to_owned(), SignalKind::Data, PortDir::Out, 0, "APB Read Data".to_owned()),
                PortInfo::new_out("pready".to_owned(), "APB Ready".to_owned()),
                PortInfo::new_out("pslverr".to_owned(), "APB Slave Error".to_owned()),
            ],
            Interface::Uaux => vec![
                PortInfo::new("uaux_addr".to_owned(), SignalKind::Address, PortDir::In, 0, "AUX Address".to_owned()),
                PortInfo::new_in("uaux_en".to_owned(), "AUX Enable".to_owned()),
                PortInfo::new_in("uaux_cmt_phase".to_owned(), "AUX Commit status".to_owned()),
                PortInfo::new_in("uaux_cmt_valid".to_owned(), "AUX Commit Valid".to_owned()),
                PortInfo::new_in("uaux_read".to_owned(), "AUX Read".to_owned()),
                PortInfo::new_in("uaux_write".to_owned(), "AUX Write".to_owned()),
                PortInfo::new("uaux_wdata".to_owned(), SignalKind::Data, PortDir::In, 0, "AUX Write Data".to_owned()),
                PortInfo::new("uaux_rdata".to_owned(), SignalKind::Data, PortDir::Out, 0, "AUX Read Data".to_owned()),
                PortInfo::new_out("uaux_busy".to_owned(), "AUX Busy".to_owned()),
                PortInfo::new_out("uaux_illegal".to_owned()  , "SR/LR illegal".to_owned()),
                PortInfo::new_out("uaux_k_rd".to_owned()     , "AUX read privilege violation".to_owned()),
                PortInfo::new_out("uaux_k_wr".to_owned()     , "AUX write privilege violation".to_owned()),
                PortInfo::new_out("uaux_unimpl".to_owned()   , "AUX unimplemented address".to_owned()),
                PortInfo::new_out("uaux_serial_sr".to_owned(), "AUX SR group flush ".to_owned()),
                PortInfo::new_out("uaux_strict_sr".to_owned(), "AUX SR single flush".to_owned()),
            ],
            Interface::Custom(name) => vec![
                PortInfo::new_intf(
                    format!("if_{}", name.strip_suffix("_if").unwrap_or(name)),
                    name.to_owned(), "rif".to_owned(),
                    "SW register interface".to_owned())
            ],
        };
        RifIntfPorts(ports)
    }

    pub fn iter(&self) -> impl Iterator<Item=&PortInfo> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item=&mut PortInfo> {
        self.0.iter_mut()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SignalRange {
    pub lsb: u8,
    pub msb: Option<u8>
}

impl SignalRange {
    pub fn new(lsb: u8, msb: u8) -> Self {
        SignalRange {
            lsb,
            msb: if msb>lsb {Some(msb)} else {None}
        }
    }

    pub fn new_bit(lsb: u8) -> Self {
        SignalRange { lsb, msb: None }
    }

    pub fn from_field(field: &RifFieldInst, fallback: bool) -> Option<SignalRange> {
        if let (Some(lsb),_) = &field.partial {
            let lsb = *lsb as u8;
            if field.width > 1 {
                Some(SignalRange {
                    lsb,
                    msb: Some(field.width - 1 + lsb) }
                )
            } else {
                Some(SignalRange::new_bit(lsb))
            }
        } else if fallback {
            // if let ArrayIdx::Inst(idx,_) = field.array {
            if field.array.dim() > 0 {
                Some(SignalRange::new_bit(field.array.idx() as u8))
            } else {
                None
            }
        } else {
            None
        }
    }
}


#[derive(Clone, Debug, PartialEq)]
/// Hardware Identifier with optional array selection, field and range selection
pub struct ExprId {
    pub name: String,
    pub idx: Option<u16>,
    pub field: Option<String>,
    pub range: Option<SignalRange>,
}

impl ExprId {
    /// Create a basic ID
    pub fn new(name: String) -> Self {
        ExprId { name, idx: None, field: None, range: None }
    }

    /// Create a simple ID with bit/array selection
    pub fn new_idx(name: String, idx: u16) -> Self {
        ExprId { name, idx: Some(idx), field: None, range: None }
    }

    /// Create a simple ID with range selection
    pub fn new_range(name: String, range: Option<SignalRange>) -> Self {
        ExprId { name, idx: None, field: None, range }
    }

    /// Create a full ID with idx, field and optional range
    pub fn new_field_range(name: String, idx: Option<u16>, field: String, range: Option<SignalRange>) -> Self {
        ExprId { name, idx, field: Some(field), range }
    }

    /// Create a new ExprId by setting the field name and range
    pub fn with_path(&self, field: String, range: Option<SignalRange>) -> Self {
        ExprId { name: self.name.clone(), idx: self.idx, field: Some(field), range }
    }

    /// Create a new ExprId by suffixing the field name
    pub fn with_fsuffix(&self, suffix: &str) -> Self {
        let field = self.field.as_ref().map(|n| format!("{n}{suffix}"));
        ExprId {
            name: self.name.clone(),
            idx: self.idx,
            field,
            range: self.range.clone()
        }
    }

    /// Create a new ExprId by replacing the field name
    pub fn with_name(&self, name: String) -> Self {
        ExprId {
            name,
            idx: self.idx,
            field: self.field.clone(),
            range: self.range.clone()
        }
    }

    /// Create a new ExprId by suffixing the field name
    pub fn with_nsuffix(&self, suffix: &str) -> Self {
        let name = format!("{}{suffix}", self.name);
        ExprId {
            name,
            idx: self.idx,
            field: self.field.clone(),
            range: self.range.clone()
        }
    }

    /// Create a new ExprId with a different range
    pub fn with_range(&self, range: SignalRange) -> Self {
        ExprId {
            name: self.name.clone(),
            idx: self.idx,
            field: self.field.clone(),
            range: Some(range)
        }
    }

    /// Create a new ExprId with a different index
    pub fn with_idx(&self, idx: u16) -> Self {
        ExprId {
            name: self.name.clone(),
            idx: Some(idx),
            field: self.field.clone(),
            range: self.range.clone()
        }
    }
}

impl From<String> for ExprId {
    fn from(value: String) -> Self {
        ExprId::new(value)
    }
}

impl From<&str> for ExprId {
    fn from(value: &str) -> Self {
        ExprId::new(value.to_owned())
    }
}

impl From<(String,Option<u16>)> for ExprId {
    fn from(value: (String,Option<u16>)) -> Self {
        ExprId { name: value.0, idx: value.1, field: None, range: None }
    }
}

impl From<(&str,&str)> for ExprId {
    fn from(value: (&str,&str)) -> Self {
        ExprId { name: value.0.to_owned(), idx: None, field: Some(value.1.to_owned()), range: None }
    }
}

impl From<(String,String)> for ExprId {
    fn from(value: (String,String)) -> Self {
        ExprId { name: value.0, idx: None, field: Some(value.1), range: None }
    }
}




#[derive(Clone, Debug, Default, PartialEq)]
pub enum CastInfo { #[default]
    /// No casting
    None,
    /// Cast to an unsigned value
    Unsigned,
    /// Cast to a signed value
    Signed,
    /// Casting to a custom type (.1) defined in a package (.0)
    Custom(String,String)
}

impl CastInfo {
    pub fn is_none(&self) -> bool {
        *self==CastInfo::None
    }
    pub fn is_custom(&self) -> bool {
        matches!(self, CastInfo::Custom(_,_))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogicExpr {
    /// Identifier with optional range information
    Id(ExprId),
    /// Cast a value to a scope::type
    Cast(CastInfo, Box<LogicExpr>),
    /// Unsigned value with width
    ValueU(u128, usize),
    /// Signed value with width
    ValueI(i128, usize),
    /// Logical not operator
    Not(Box<LogicExpr>),
    /// Bitwise not operator
    NotB(Box<LogicExpr>),
    /// Concatenation of logic expression into a logic vector
    Concat(Vec<LogicExpr>),
    /// Logical or between n logic expression
    Or(Vec<LogicExpr>),
    /// Logical and between n logic expression
    And(Vec<LogicExpr>),
    /// Addition between two logic expression
    Add(Box<LogicExpr>,Box<LogicExpr>),
    /// Subtraction between two logic expression
    Sub(Box<LogicExpr>,Box<LogicExpr>),
    /// Equality between two logic expression
    Eq(Box<LogicExpr>,Box<LogicExpr>),
    /// Inequality between two logic expression
    Neq(Box<LogicExpr>,Box<LogicExpr>),
    /// Xor between two Id
    Xor(Box<LogicExpr>,Box<LogicExpr>),
    /// Bitwise Or between two Id
    OrB(Box<LogicExpr>,Box<LogicExpr>),
    /// Bitwise And between two Id
    AndB(Box<LogicExpr>,Box<LogicExpr>),
    /// Greater Than or Equal between two logic expression
    Gte(Box<LogicExpr>,Box<LogicExpr>),
    /// Lesser Than or Equal between two logic expression
    Lte(Box<LogicExpr>,Box<LogicExpr>),
    /// Lesser Than or Equal between two logic expression
    Lt(Box<LogicExpr>,Box<LogicExpr>),
    /// Chain of If/Then/Else, last argument is the final else
    Ite(Vec<(LogicExpr,LogicExpr)>,Box<LogicExpr>),
}

impl From<String> for LogicExpr {
    fn from(value: String) -> Self {
        LogicExpr::Id(ExprId::from(value))
    }
}

impl From<&str> for LogicExpr {
    fn from(value: &str) -> Self {
        LogicExpr::Id(ExprId::from(value))
    }
}

impl From<(&str, &str)> for LogicExpr {
    fn from(value: (&str, &str)) -> Self {
        LogicExpr::Id(ExprId::from(value))
    }
}

impl From<(&str, &str, Option<SignalRange>)> for LogicExpr {
    fn from(value: (&str, &str, Option<SignalRange>)) -> Self {
        LogicExpr::Id(ExprId{
            name: value.0.to_owned(), idx: None, field: Some(value.1.to_owned()), range: value.2
        })
    }
}

impl From<ExprId> for LogicExpr {
    fn from(value: ExprId) -> Self {
        LogicExpr::Id(value)
    }
}

impl From<&ExprId> for LogicExpr {
    fn from(value: &ExprId) -> Self {
        LogicExpr::Id(value.clone())
    }
}

impl From<&MissingFieldInfo> for LogicExpr {
    fn from(value: &MissingFieldInfo) -> Self {
        if value.signed {
            let mut rst = value.reset as i128;
            if rst >= (1<<(value.width-1)) {
                rst -= 1<<value.width;
            }
            LogicExpr::ValueI(rst, value.width.into())
        } else {
            LogicExpr::ValueU(value.reset, value.width.into())
        }
    }
}

impl LogicExpr {
    /// Create a Value (Signed/Unsigned) from a u128 and a field definition (for signed and width)
    pub fn value(v: u128, info: &RifFieldInst) -> LogicExpr {
        if info.is_signed() {
            LogicExpr::ValueI(v as i128, info.width.into())
        } else {
            LogicExpr::ValueU(v, info.width.into())
        }
    }
    /// Create a Value (Signed/Unsigned) from a u128 and a field definition (for signed and width)
    pub fn reset(v: &ResetVal, width: u8) -> LogicExpr {
        if let ResetVal::Signed(vi) = v {
            LogicExpr::ValueI(*vi, width.into())
        } else {
            LogicExpr::ValueU(v.to_u128(width), width.into())
        }
    }
    pub fn and(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::And(vec![lhs, rhs])
    }
    pub fn or(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Or(vec![lhs, rhs])
    }
    pub fn and_b(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::AndB(Box::new(lhs), Box::new(rhs))
    }
    pub fn or_b(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::OrB(Box::new(lhs), Box::new(rhs))
    }
    #[allow(clippy::should_implement_trait)]
    pub fn not(expr: LogicExpr) -> LogicExpr {
        LogicExpr::Not(Box::new(expr))
    }
    pub fn not_b(expr: LogicExpr) -> LogicExpr {
        LogicExpr::NotB(Box::new(expr))
    }
    pub fn eq(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Eq(Box::new(lhs), Box::new(rhs))
    }
    #[allow(clippy::should_implement_trait)]
    pub fn add(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Add(Box::new(lhs), Box::new(rhs))
    }
    #[allow(clippy::should_implement_trait)]
    pub fn sub(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Sub(Box::new(lhs), Box::new(rhs))
    }
    pub fn neq(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Neq(Box::new(lhs), Box::new(rhs))
    }
    pub fn xor(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Xor(Box::new(lhs), Box::new(rhs))
    }
    pub fn ite(cond: LogicExpr, if_true: LogicExpr, if_false: LogicExpr) -> LogicExpr {
        LogicExpr::Ite([(cond,if_true)].to_vec(), Box::new(if_false))
    }
    pub fn gte(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Gte(Box::new(lhs), Box::new(rhs))
    }
    pub fn lte(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Lte(Box::new(lhs), Box::new(rhs))
    }
    pub fn lt(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Lt(Box::new(lhs), Box::new(rhs))
    }

    pub fn push(&mut self, expr: LogicExpr) {
        match self {
            LogicExpr::Or(vec) => vec.push(expr),
            LogicExpr::And(vec) => vec.push(expr),
            _ => panic!("Illegal push call on expression {self:?}")
        }
    }

    pub fn add_ite(&mut self, cond: LogicExpr, value: LogicExpr) {
        match self {
            LogicExpr::Ite(vec, _) => vec.push((cond,value)),
            _ => panic!("Illegal add_ite call on expression {self:?}")
        }
    }

    pub fn width(&self) -> usize {
        match self {
            LogicExpr::ValueU(_, w) => *w,
            LogicExpr::ValueI(_, w) => *w,
            _ => 0,
        }
    }

    pub fn is_id(&self, name: &str) -> bool {
        match self {
            LogicExpr::Id(id) => id.name == name,
            _ => false
        }
    }

    pub fn is_value(&self) -> bool {
        matches!(*self,LogicExpr::ValueU(_,_) | LogicExpr::ValueI(_,_))
    }

    pub fn is_ite(&self) -> bool {
        matches!(*self, LogicExpr::Ite(_,_))
    }

    pub fn is_else_value(&self) -> bool {
        match self {
            LogicExpr::Ite(_, else_expr) => else_expr.is_value(),
            _ => false
        }
    }

    pub fn is_and(&self) -> bool {
        matches!(*self, LogicExpr::And(_))
    }

    pub fn has_vec(&self) -> bool {
        matches!(*self, LogicExpr::Ite(_,_) | LogicExpr::Or(_) | LogicExpr::And(_) )
    }

    pub fn is_comp(&self) -> bool {
        matches!(*self, LogicExpr::Gte(_,_) | LogicExpr::Lte(_,_) | LogicExpr::Lt(_,_) | LogicExpr::Neq(_,_) | LogicExpr::Eq(_,_))
    }

    pub fn has_comp(&self) -> bool {
        match self {
            LogicExpr::Eq(_,_)  |
            LogicExpr::Neq(_,_)  |
            LogicExpr::Gte(_,_)  |
            LogicExpr::Lte(_,_)  |
            LogicExpr::Lt(_,_)   => true,
            LogicExpr::Or(vec)  |
            LogicExpr::And(vec) => vec.iter().any(|e| e.is_comp()),
            _ => false
        }
    }

    /// Number of expressions
    pub fn nb(&self) -> usize {
        match self {
            LogicExpr::Or(vec)     => vec.len(),
            LogicExpr::And(vec)    => vec.len(),
            LogicExpr::Ite(vec, _) => vec.len(),
            LogicExpr::Concat(vec) => vec.len(),
            LogicExpr::Id(_)       |
            LogicExpr::Cast(_,_)   |
            LogicExpr::ValueU(_,_) |
            LogicExpr::ValueI(_,_) |
            LogicExpr::Not(_)      |
            LogicExpr::NotB(_)     => 1,
            LogicExpr::Add(_,_)  |
            LogicExpr::Sub(_,_)  |
            LogicExpr::Eq(_,_)   |
            LogicExpr::Neq(_,_)  |
            LogicExpr::Xor(_,_)  |
            LogicExpr::OrB(_,_)  |
            LogicExpr::AndB(_,_) |
            LogicExpr::Gte(_,_)  |
            LogicExpr::Lte(_,_)  |
            LogicExpr::Lt(_,_)   => 2,
        }
    }

}