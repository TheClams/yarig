use std::path::PathBuf;

use crate::{
    comp::{comp_inst::RifPageInst, reg_impl::{FieldImpl, HwRegs, RegImplDict}},
    error::RifGenError, generator::casing::{Casing, ToCasing},
    hdl::{ExprId,LogicExpr, parser_sv::sv_module_decl},
    rifgen::{ClkEn, FieldHwKind, Interface, Rif, SuffixInfo}
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
            let w = if f.sw_kind.is_password() {2} else {f.width()};
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

#[derive(Clone, Debug, PartialEq)]
pub enum SignalDim {
    Fixed(u16),
    Generic(String, u16)
}

impl SignalDim {
    pub fn is_null(&self) -> bool {
        matches!(self, SignalDim::Fixed(0))
    }

    pub fn is_array(&self) -> bool {
        !self.is_null()
    }

    pub fn val(&self) -> u16 {
        match self {
            SignalDim::Fixed(d) => *d,
            SignalDim::Generic(_, d) => *d,
        }
    }
}

// Default dimension to single dimension
impl Default for SignalDim {
    fn default() -> Self {
        SignalDim::Fixed(0)
    }
}

impl std::fmt::Display for SignalDim {
    /// Display the dimension as the fixed number of the name of the generic
    /// The alternate flag print dimension-1 (usefull to get the MSB as string)
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignalDim::Fixed(d) => {
                if f.alternate() {write!(f,"{}",d-1)}
                else {write!(f,"{d}")}
            }
            SignalDim::Generic(n, _) => {
                if f.alternate() {write!(f,"{n}-1")}
                else {write!(f,"{n}")}
            }
        }
    }
}

impl From<u16> for SignalDim {
    fn from(value: u16) -> Self {
        SignalDim::Fixed(value)
    }
}

/// Signal declaration info: contains both name and type
#[derive(Clone, Debug)]
pub struct SignalDef {
    /// Signal name
    pub name: String,
    /// Signal kind (unsigned/signed/custom type)
    pub kind: SignalKind,
    /// Array dimension (0 means not an array)
    pub dim: SignalDim,
}

impl SignalDef {
    /// Direct constructor
    pub fn new_arr(name: String, kind: SignalKind, dim: SignalDim) -> Self {
        SignalDef { name, kind, dim}
    }

    /// Simple Signal
    pub fn new(name: String, kind: SignalKind) -> Self {
        SignalDef { name, kind, dim: SignalDim::Fixed(0)}
    }

    /// Single bit signal
    pub fn new_bit(name: String) -> Self {
        SignalDef { name, kind: SignalKind::Unsigned(1), dim: SignalDim::Fixed(0)}
    }

    /// Bus signal
    pub fn new_bus(name: String, width: u16, signed: bool) -> Self {
        SignalDef { name, kind: SignalKind::new(width, signed), dim: SignalDim::Fixed(0)}
    }

    /// Integer signal
    pub fn new_int(name: String) -> Self {
        SignalDef { name, kind: SignalKind::Integer, dim: SignalDim::Fixed(0)}
    }

    /// Define a signal with User-define type
    pub fn new_ud(name: String, scope: String, type_name: String) -> Self {
        SignalDef {
            name,
            kind: SignalKind::Custom((Some(scope), type_name)),
            dim: SignalDim::Fixed(0)
        }
    }

    /// Define a signal with User-define type
    pub fn new_ud_arr(name: String, scope: String, type_name: String, dim: SignalDim) -> Self {
        SignalDef {
            name,
            kind: SignalKind::Custom((Some(scope), type_name)),
            dim
        }
    }

    /// Check is signal is an array
    pub fn is_array(&self) -> bool {
        self.dim.is_array()
    }
}

impl PartialEq for SignalDef {
    fn eq(&self, other: &Self) -> bool {
        self.name.trim() == other.name.trim() &&
        self.kind == other.kind &&
        self.dim == other.dim
    }
}

/// Signal declaration info: contains both name and type
#[derive(Clone, Debug)]
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
        let name = if let Some(e) = kind.get_signal() {
            let n = e.local_field(regname)?;
            n.to_owned()
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

#[derive(Debug)]
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
            def: SignalDef::new(name, SignalKind::Custom((None,if_name))),
            dir: PortDir::Modport(modport),
            desc,
        }
    }

    pub fn new_rif_intf(is_rif: bool) -> Self {
        let modport = if is_rif {"rif"} else {"ctrl"};
        PortInfo {
            def: SignalDef::new("if_rif".to_owned(), SignalKind::Custom((None,"rif_if".to_owned()))),
            dir: PortDir::Modport(modport.to_owned()),
            desc: "SW register interface".to_owned(),
        }
    }

    pub fn new(name: String, kind: SignalKind, dir: PortDir, dim: SignalDim, desc: String) -> Self {
        PortInfo {
            def: SignalDef::new_arr(name, kind, dim),
            dir,
            desc,
        }
    }

    pub fn new_basic(name: String, kind: SignalKind, dir: PortDir, desc: String) -> Self {
        PortInfo {
            def: SignalDef::new(name, kind),
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

    pub fn dim(&self) -> &SignalDim {
        &self.def.dim
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

impl PartialEq for PortInfo {
    fn eq(&self, other: &Self) -> bool {
        self.def == other.def &&
        self.dir == other.dir &&
        self.desc.trim() == other.desc.trim()
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

impl PortList {
    pub fn new(rif: &Rif, pages_inst: &[RifPageInst], regs_impls: &RegImplDict, hw_regs: &HwRegs, suffix: &Option<SuffixInfo>) -> Self {
        let mut clocks : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut resets : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut clk_ens : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut ctrls  : Vec<PortInfo> = Vec::with_capacity(2);
        let mut regs   : Vec<PortInfo> = Vec::with_capacity(hw_regs.len());
        let mut irqs   : Vec<PortInfo> = Vec::with_capacity(2) ;
        // Software clocking
        let sw_clocking = rif.sw_clocking.last().cloned().unwrap_or_default();
        clocks.push(PortInfo::new_in(sw_clocking.clk.to_owned(), "Software clock".to_owned()));
        resets.push(PortInfo::new_in(
            sw_clocking.rst.name.to_owned(),
            format!("Software {}", sw_clocking.rst.desc())
        ));
        if !sw_clocking.en.is_empty() {
            clk_ens.push(PortInfo::new_in(sw_clocking.en.to_owned(), "Software clock enable".to_owned()))
        }
        if !sw_clocking.clear.is_empty() {
            ctrls.push(PortInfo::new_in(
                sw_clocking.clear.to_owned(),
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
                    format!("Hardware {}", sw_clocking.rst.desc())));
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
            if let ClkEn::Signal(en) = &r.clk_en && !clk_ens.iter().any(|p| p.name()==en) {
                clk_ens.push(PortInfo::new_in(en.to_owned(), "Clock enable".to_owned()));
            }
            for f in r.fields.iter() {
                if let ClkEn::Signal(en) = &f.clk_en && !clk_ens.iter().any(|p| p.name()==en) {
                    clk_ens.push(PortInfo::new_in(en.to_owned(), "Clock enable".to_owned()));
                }
                if let Some(lock) = f.lock.port_name() && !ctrls.iter().any(|p| p.name()==lock) {
                    ctrls.push(PortInfo::new_in(lock.to_owned(), "Lock signal".to_owned()));
                }
                for k in f.hw_kind.iter() {
                    if let Some(sig) = k.get_signal()
                        && let Some(port) = sig.port_name()
                        && !ctrls.iter().any(|p| p.name()==port) {
                            ctrls.push(PortInfo::new_in(port.to_owned(), "Control signal".to_owned()));
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
            let desc = hw_reg_def.description.get_short(false);
            if hw_reg.port.is_in() {
                let port = PortInfo::new(
                    group_name.to_owned(),
                    SignalKind::Custom((Some(pkg_name.clone()), format!("t_{group_type}_hw"))),
                    PortDir::In,
                    hw_reg.dim.clone(),
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
                    hw_reg.dim.clone(),
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
            Interface::Default => Self::default_ports(use_intf),
            Interface::Apb => vec![
                PortInfo::new_basic("paddr  ".to_owned(), SignalKind::Address, PortDir::In, "APB Address".to_owned()),
                PortInfo::new_in(   "psel   ".to_owned(), "APB Select".to_owned()),
                PortInfo::new_in(   "penable".to_owned(), "APB Enable".to_owned()),
                PortInfo::new_in(   "pwrite ".to_owned(), "APB Write".to_owned()),
                PortInfo::new_basic("pwdata ".to_owned(), SignalKind::Data, PortDir::In, "APB Write Data".to_owned()),
                PortInfo::new_basic("prdata ".to_owned(), SignalKind::Data, PortDir::Out, "APB Read Data".to_owned()),
                PortInfo::new_out(  "pready ".to_owned(), "APB Ready".to_owned()),
                PortInfo::new_out(  "pslverr".to_owned(), "APB Slave Error".to_owned()),
            ],
            Interface::Uaux => vec![
                PortInfo::new_basic("uaux_addr     ".to_owned(), SignalKind::Address, PortDir::In, "AUX address".to_owned()),
                PortInfo::new_in(   "uaux_en       ".to_owned(), "AUX enable".to_owned()),
                PortInfo::new_in(   "uaux_cmt_phase".to_owned(), "AUX commit status".to_owned()),
                PortInfo::new_in(   "uaux_cmt_valid".to_owned(), "AUX commit valid".to_owned()),
                PortInfo::new_in(   "uaux_read     ".to_owned(), "AUX read".to_owned()),
                PortInfo::new_in(   "uaux_write    ".to_owned(), "AUX write".to_owned()),
                PortInfo::new_basic("uaux_wdata    ".to_owned(), SignalKind::Data, PortDir::In, "AUX write data".to_owned()),
                PortInfo::new_basic("uaux_rdata    ".to_owned(), SignalKind::Data, PortDir::Out, "AUX read data".to_owned()),
                PortInfo::new_out(  "uaux_busy     ".to_owned(), "AUX busy".to_owned()),
                PortInfo::new_out(  "uaux_illegal  ".to_owned(), "SR/LR illegal".to_owned()),
                PortInfo::new_out(  "uaux_k_rd     ".to_owned(), "AUX read privilege violation".to_owned()),
                PortInfo::new_out(  "uaux_k_wr     ".to_owned(), "AUX write privilege violation".to_owned()),
                PortInfo::new_out(  "uaux_unimpl   ".to_owned(), "AUX unimplemented address".to_owned()),
                PortInfo::new_out(  "uaux_serial_sr".to_owned(), "AUX SR group flush ".to_owned()),
                PortInfo::new_out(  "uaux_strict_sr".to_owned(), "AUX SR single flush".to_owned()),
            ],
            Interface::Custom(name,_) => vec![
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

    pub fn default_ports( use_intf: bool) -> Vec<PortInfo> {
        if use_intf {
            vec![PortInfo::new_rif_intf(true)]
        } else {
            vec![
                PortInfo::new_basic("reg_addr           ".to_owned(), SignalKind::Address, PortDir::In, "Register address".to_owned()),
                PortInfo::new_in(   "reg_en             ".to_owned(), "Register enable".to_owned()),
                PortInfo::new_in(   "reg_rd_wrn         ".to_owned(), "Register write/not read".to_owned()),
                PortInfo::new_basic("reg_wr_data        ".to_owned(), SignalKind::Data, PortDir::In , "Register write data".to_owned()),
                PortInfo::new_basic("reg_rd_data        ".to_owned(), SignalKind::Data, PortDir::Out, "Register read data".to_owned()),
                PortInfo::new_out(  "reg_done           ".to_owned(), "Register ready".to_owned()),
                PortInfo::new_out(  "reg_done_next      ".to_owned(), "Register ready next".to_owned()),
                PortInfo::new_out(  "reg_err_addr       ".to_owned(), "Register address error".to_owned()),
                PortInfo::new_out(  "reg_err_addr_next  ".to_owned(), "Register address error (combinatorial)".to_owned()),
                PortInfo::new_out(  "reg_err_access     ".to_owned(), "Register access error".to_owned()),
                PortInfo::new_out(  "reg_err_access_next".to_owned(), "Register access error (combinatorial)".to_owned()),
            ]
        }
    }

}


/// Hardware block parameter information: name, kind, type, default value
#[derive(Clone, Debug, Default)]
pub struct ParamInfo {
    pub kind: ParamKind,
    pub ptype: ParamType,
    pub name: String,
    pub value: isize,
    pub desc: String,
}

pub type ParamDecl<'a> = (Option<ParamKind>, Option<ParamType>, &'a str, Option<isize>, Option<&'a str>);

impl ParamInfo {
    /// Create a basic ParamInfo with just a type and a name
    pub fn basic(ptype: ParamType, name: String, value: isize) -> Self {
        Self{kind: ParamKind::Global, ptype, name, value, desc: String::new() }
    }

    /// Update a ParamInfo from the result of paramdeclaration parsing
    /// During parsing kind and type might be omitted and should take the value of the previous param declaration
    pub fn set(&mut self, decl: ParamDecl) {
        if let Some(k) = decl.0 {self.kind = k;}
        if let Some(t) = decl.1 {self.ptype = t;}
        self.name = decl.2.to_owned();
        self.value = decl.3.unwrap_or_default();
        if let Some(d) = decl.4 {self.desc = d.to_owned();}
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ParamKind { #[default]
    Global, Local
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ParamType { #[default]
    Int, Logic(u16), Bit(u16)
}

impl PartialEq for ParamInfo {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind &&
        self.ptype == other.ptype &&
        self.name.trim() == other.name.trim() &&
        self.value == other.value &&
        self.desc.trim() == other.desc.trim()
    }
}


/// Hardware block parameter information: name, kind, type, default value
#[derive(Clone, Debug)]
pub struct ModuleInfo {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub ports: Vec<PortInfo>,
}

impl ModuleInfo {

    /// Create Module info from one of the non-custom supported bridge
    pub fn new_bridge(intf: &Interface) -> Self {
        let mut ports : Vec<PortInfo> = Vec::new();
        if intf==&Interface::Uaux {
            ports.push(PortInfo::new_in("clk".to_owned(), "SW Clock".to_owned()));
            ports.push(PortInfo::new_in("rst_n".to_owned(), "SW reset asynchronous, active low".to_owned()));
        }
        ports.push(PortInfo::new_rif_intf(true));
        if intf!=&Interface::Default {
            ports.extend(RifIntfPorts::new(intf, true).0);
        }
        let params = if intf==&Interface::Default {Vec::new()}
            else {vec![
                ParamInfo::basic(ParamType::Int, "ADDR_W".to_owned(), 16),
                ParamInfo::basic(ParamType::Int, "DATA_W".to_owned(), 32),
            ]};
        let name = format!("bridge_{}_rif", intf.name());
        Self {name, params, ports}
    }

    /// Create Module info from one of the non-custom supported bridge
    pub fn from_file <T : Into<PathBuf>>(path: T) -> Result<Self, RifGenError> {
        let file_content = std::fs::read_to_string(path.into())?;
        let mut txt = file_content.as_str();
        let info = sv_module_decl(&mut txt)?;
        Ok(Self {
            name: info.0.to_owned(),
            params: info.1,
            ports: info.2,
        })
    }

    /// Return software ports excluding the Clock/reset interface
    pub fn get_ports_sw(&self) -> Vec<PortInfo> {
        let rif_if_en = self.ports.len()==1; // Only one port os the mark of default interface
        // Exclude port if_rif, reg_*, clk/clock, rst, reset
        let filter = |p: &PortInfo| ((p.name()!="if_rif" && !p.name().starts_with("reg_")) || rif_if_en)
            && !p.name().contains("clk") && !p.name().contains("clock")
            && !p.name().contains("rst") && !p.name().contains("reset") ;
        self.ports.iter().filter(|&p| filter(p)).cloned().collect::<Vec<_>>()
    }

    /// Check if the interface contains clock signal (naming containing clk/clock)
    pub fn has_clk(&self) -> bool {
        self.ports.iter().any(|p|
            p.name().contains("clk") || p.name().contains("clock")
        )
    }

}
