use std::{collections::HashMap, fmt::Display, ops::{Add, Sub}};

use crate::{error::RifError, parser::parser_expr::{ExprTokens, ParamValues}, rifgen::GenericValues};
use crate::hdl::LogicExpr;

use super::{Context, Description, InterruptClr, InterruptDesc, InterruptInfoField, InterruptRegKind, InterruptTrigger, DeclLine, DescBlockKind, PropBlocks, PropLines};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PulseLogic {
    Comb,
    Reg,
    ResyncSimple,
    ResyncAsync,
}

/// Generic access
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Access {
	/// Read/Write
    RW,
    /// Read-Only
    RO,
    /// Write-Only
    WO,
    /// Not Available
    #[default] NA,
}

impl Access {

    pub fn updt(&mut self, access: Access) {
        match self {
            Access::NA => {*self = access},
            Access::RO => if access==Access::WO || access==Access::RW {*self = Access::RW},
            Access::WO => if access==Access::RO || access==Access::RW {*self = Access::RW},
            Access::RW => {},
        }
    }

    /// True if access is RW or WO
    pub fn is_writable(&self) -> bool {
        match self {
            Access::RW | Access::WO => true,
            Access::RO | Access::NA => false,
        }
    }

    /// True if access is RW or RO
    pub fn is_readable(&self) -> bool {
        match self {
            Access::RW | Access::RO => true,
            Access::WO | Access::NA => false,
        }
    }

    /// True if access is NA
    pub fn is_na(&self) -> bool {
        matches!(self, Access::NA)
    }

    /// True if access is NA
    pub fn exclusive(&self, other: Access) -> bool {
        (*self==Access::RO && other==Access::WO) ||
        (*self==Access::WO && other==Access::RO)
    }
}

impl From<&FieldSwKind> for Access {
    fn from(value: &FieldSwKind) -> Self {
        match value {
            FieldSwKind::ReadWrite => Access::RW,
            FieldSwKind::WriteOnly |
            FieldSwKind::W1Pulse(_,true) => Access::WO,
            FieldSwKind::ReadOnly => Access::RO,
            FieldSwKind::ReadClr => Access::RO,
            _ => Access::RW
        }
    }
}

impl From<&FieldHwKind> for Access {
    fn from(value: &FieldHwKind) -> Self {
        match value {
            FieldHwKind::ReadOnly => Access::RO,
            _ => Access::WO,
        }
    }
}

impl Display for Access {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Access::RW => write!(f, "RW"),
            Access::RO => write!(f, "RO"),
            Access::WO => write!(f, "WO"),
            Access::NA => write!(f, "NA"),
        }
    }
}


#[derive(Clone, Debug, PartialEq, Default)]
/// Enumerated value entry : name, value, floating representation and description
pub struct EnumEntry {
    /// Name
    pub name: String,
    /// Integer value
    pub value: u8,
    /// Optional flaoting representation
    pub repr: Option<f64>,
    /// Description
    pub description: Description,
    /// Source-tracking metadata for round-tripping edits back to the `.rif` file.
    pub src: DeclLine,
}

impl EnumEntry {
    /// Serialize this entry to its `.rif` source line (without leading indent):
    /// `- name = value (repr) "description".
    pub fn to_rif(&self) -> String {
        let mut s = format!("- {} = {}", self.name, self.value);
        if let Some(r) = self.repr {
            s.push_str(&format!(" ({r})"));
        }
        s.push_str(&format!(" \"{}\"", self.description.get_short(false)));
        s
    }
}

#[derive(Clone, Debug)]
pub struct EnumDef {
    /// Enum type name
    pub name: String,
    /// Enum description
    pub description: String,
    /// List of all enum variants
    pub values: Vec<EnumEntry>,
    /// Source-tracking metadata for edition.
    pub src: DeclLine,
}

impl EnumDef {
    pub fn new(name: String, description: String) -> Self {
        EnumDef {name, description, values: Vec::with_capacity(4), src: DeclLine::default()}
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item=&EnumEntry> {
        self.values.iter()
    }

    pub fn is_local_type(&self) -> bool {
        !self.name.contains(':')
    }

    pub fn get(&self, name: &str) -> Option<&EnumEntry> {
        self.values.iter().find(|e| e.name==name)
    }

    pub fn list(&self) -> Vec<&String> {
        self.values.iter().map(|e| &e.name).collect()
    }
}

#[derive(Clone, Debug)]
pub struct EnumDefs(Vec<EnumDef>);

impl From<Vec<EnumDef>> for EnumDefs {
    fn from(value: Vec<EnumDef>) -> Self {
        EnumDefs(value)
    }
}

impl EnumDefs {
    /// Add enum definition
    pub fn push(&mut self, def: EnumDef) {
        self.0.push(def)
    }

    /// Add enum definition
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Add enum definition
    pub fn iter(&self) -> std::slice::Iter<'_, EnumDef> {
        self.0.iter()
    }

    /// Retrieve an enum definition
    pub fn find(&self, name: &str) -> Option<&EnumDef> {
        self.0.iter().find(|e| e.name == name)
    }

    pub fn list(&self) -> Vec<&String> {
        self.0.iter().map(|def| &def.name).collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CounterKind {
    Up,
    Down,
    UpDown,
}

#[derive(Clone, Debug, PartialEq)]
/// Counter definition: Up/Down with optional external incr/decr value, overflow handling and clear signal
pub struct CounterInfo {
    pub kind: CounterKind,
    pub incr_val: u8,
    pub decr_val: u8,
    pub sat: bool,
    pub clr: bool,
    pub event: bool,
}

impl CounterInfo {
    /// True when counter can increment
    pub fn is_up(&self) -> bool {
        matches!(self.kind, CounterKind::Up | CounterKind::UpDown)
    }

    /// True when counter can decrement
    pub fn is_down(&self) -> bool {
        matches!(self.kind, CounterKind::Down | CounterKind::UpDown)
    }

    /// True when counter can saturate and incr/decr are not single bit
    pub fn has_satn(&self) -> bool {
        self.sat && !self.is_single_bit()
    }

    /// True when counter incr/decr are single bit
    pub fn is_single_bit(&self) -> bool {
        self.incr_val <= 1 && self.decr_val <= 1
    }

    /// Serialize the counter to `.rif` source (without the `counter` keyword):
    /// `up|down|updown [incrVal=<n>] [decrVal=<n>] [sat] [event] [clr]`.
    pub fn to_rif(&self) -> String {
        let mut s = String::from(match self.kind {
            CounterKind::Up => "up",
            CounterKind::Down => "down",
            CounterKind::UpDown => "updown",
        });
        if self.incr_val != 0 { s.push_str(&format!(" incrVal={}", self.incr_val)); }
        if self.decr_val != 0 { s.push_str(&format!(" decrVal={}", self.decr_val)); }
        if self.sat { s.push_str(" sat"); }
        if self.event { s.push_str(" event"); }
        if self.clr { s.push_str(" clr"); }
        s
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum VisibilityRaw {#[default]
    Full,
    Hidden,
    Reserved,
    Disabled(ExprTokens),
}

impl VisibilityRaw {
    pub fn compile(&self, params: &ParamValues) -> Visibility {
        match self {
            VisibilityRaw::Full => Visibility::Full,
            VisibilityRaw::Hidden => Visibility::Hidden,
            VisibilityRaw::Reserved => Visibility::Reserved,
            VisibilityRaw::Disabled(expr_tokens) => {
                if let Ok(v) = expr_tokens.eval(params) {
                    if v==1 {Visibility::Disabled} else {Visibility::Full}
                } else {
                    Visibility::Full
                }
            }
        }

    }
}
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Visibility {#[default]
    Full,
    Hidden,
    Reserved,
    Disabled,
    Unused,
}

impl Visibility {

    pub fn is_reserved(&self) -> bool {
        self == &Visibility::Reserved
    }
    #[allow(dead_code)]
    pub fn is_hidden(&self) -> bool {
        self == &Visibility::Hidden
    }
    #[allow(dead_code)]
    pub fn is_disabled(&self) -> bool {
        self == &Visibility::Disabled
    }

    pub fn is_unused(&self) -> bool {
        self == &Visibility::Unused
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum FieldHwKind {#[default]
    ReadOnly,
    Set(Option<LogicExpr>),
    Toggle(Option<LogicExpr>),
    Clear(Option<LogicExpr>),
    WriteEn(Option<LogicExpr>),
    WriteEnL(Option<LogicExpr>),
    Counter(CounterInfo),
    Interrupt(InterruptTrigger),
}

impl FieldHwKind {
    pub fn has_we(&self) -> bool {
        matches!(self, FieldHwKind::WriteEn(_) | FieldHwKind::WriteEnL(_))
    }

    pub fn has_write_mod(&self) -> bool {
        matches!(self,
            FieldHwKind::Set(_) | FieldHwKind::Toggle(_) | FieldHwKind::Clear(_) |
            FieldHwKind::WriteEn(_) | FieldHwKind::WriteEnL(_))
    }

    pub fn is_counter(&self)  -> bool {
        matches!(self,FieldHwKind::Counter(_))
    }

    pub fn is_interrupt(&self)  -> bool {
        matches!(self,FieldHwKind::Interrupt(_))
    }

    /// Return the signal name associated with hardware write kind (set/toggle/clear/write)
    pub fn get_signal(&self) -> &Option<LogicExpr> {
        match self {
            FieldHwKind::Set(signal) => signal,
            FieldHwKind::Toggle(signal) => signal,
            FieldHwKind::Clear(signal) => signal,
            FieldHwKind::WriteEn(signal) => signal,
            FieldHwKind::WriteEnL(signal) => signal,
            _ => &None
        }
    }

    /// Return the suffix associated with hardware write kind (set/toggle/clear/write)
    pub fn get_suffix(&self) -> &str {
        match self {
            FieldHwKind::Set(None)      => "_hwset",
            FieldHwKind::Toggle(None)   => "_tgl",
            FieldHwKind::Clear(None)    => "_hwclr",
            FieldHwKind::WriteEn(None)  => "_we",
            FieldHwKind::WriteEnL(None) => "_wel",
            _ => ""
        }
    }

    /// Return a comment string associated with hardware write kind (set/toggle/clear/write)
    pub fn get_comment(&self, name: &str) -> String {
        match self {
            FieldHwKind::Set(_)      => format!("Pulse high to set {name}"),
            FieldHwKind::Toggle(_)   => format!("Pulse high to toggle {name}"),
            FieldHwKind::Clear(_)    => format!("Pulse high to clear {name}"),
            FieldHwKind::WriteEn(_)  => format!("Pulse high to write {name}"),
            FieldHwKind::WriteEnL(_) => format!("Pulse low to write {name} "),
            _ => "".to_owned()
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq, Default)]
/// Software access kind
pub enum FieldSwKind {#[default]
    /// Read & Write
    ReadWrite,
    /// Read only
    ReadOnly,
    /// Write only
    WriteOnly,
    /// Clear on software read
    ReadClr,
    /// Clear when writing 1
    W1Clr,
    /// Clear when writing 0
    W0Clr,
    /// Set to 1 only
    W1Set,
    /// Write1 to toggle value
    W1Tgl,
    /// Generate a pulse when 1. First boolean indicate pulse is delayed by one clock, second if the field is read-only
    W1Pulse(bool, bool),
    /// Password field: no value stored but control an internal lock field
    Password(PasswordInfo)
}

impl FieldSwKind {

    pub fn is_password(&self) -> bool {
        matches!(self, FieldSwKind::Password(_))
    }

    pub fn is_clr(&self) -> bool {
        matches!(self, FieldSwKind::ReadClr | FieldSwKind::W1Clr | FieldSwKind::W0Clr)
    }

    pub fn is_set(&self) -> bool {
        matches!(self, FieldSwKind::W1Set)
    }

    pub fn is_pulse(&self) -> bool {
        matches!(self, FieldSwKind::W1Pulse(_,_))
    }

    pub fn is_pulse_comb(&self) -> bool {
        matches!(self, FieldSwKind::W1Pulse(false,_))
    }

    pub fn is_ro(&self) -> bool {
        matches!(self, FieldSwKind::ReadOnly | FieldSwKind::ReadClr)
    }

    pub fn is_wo(&self) -> bool {
        matches!(self, FieldSwKind::WriteOnly | FieldSwKind::W1Pulse(_,true) | FieldSwKind::Password(_))
    }

    /// Return true if the field kind is not read-write, read-only or write-only
    pub fn is_special(&self) -> bool {
        !matches!(self, FieldSwKind::WriteOnly | FieldSwKind::ReadWrite | FieldSwKind::ReadOnly)
    }

    /// Return access as a string (used mainly in doc generators)
    pub fn access_str(&self) -> &str {
        match self {
            FieldSwKind::ReadWrite   => "RW",
            FieldSwKind::ReadOnly    => "RO",
            FieldSwKind::WriteOnly   => "WO",
            FieldSwKind::ReadClr     => "RCLR",
            FieldSwKind::W1Clr       => "W1CLR",
            FieldSwKind::W0Clr       => "W0CLR",
            FieldSwKind::W1Set       => "W1SET",
            FieldSwKind::W1Tgl       => "W1TGL",
            FieldSwKind::W1Pulse(_,_) => "Pulse",
            FieldSwKind::Password(_) => "Password",
        }
    }

    /// Inline software-access token for a `.rif` declaration line.
    /// Return `None` when implicit or when it cannot be expressed as a single inline token
    pub fn inline_token(&self) -> Option<&'static str> {
        match self {
            FieldSwKind::ReadWrite   => None,
            FieldSwKind::ReadOnly    => Some("ro"),
            FieldSwKind::WriteOnly   => Some("wo"),
            FieldSwKind::ReadClr     => Some("rclr"),
            FieldSwKind::W1Clr       => Some("w1clr"),
            FieldSwKind::W0Clr       => Some("w0clr"),
            FieldSwKind::W1Set       => Some("w1set"),
            FieldSwKind::W1Tgl       => Some("toggle"),
            FieldSwKind::W1Pulse(false, false) => Some("pulse"),
            FieldSwKind::W1Pulse(true, false) => Some("pulsereg"),
            _ => None,
        }
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum EnumKind {
    None,
    Doc(String),
    Type(String),
}

impl EnumKind {

    pub fn new(kind: &str, reg_name: &str, field_name: &str) -> Self {
        match kind {
            // Empty kind -> document only enum
            "" => EnumKind::Doc(format!("doc:{reg_name}_{field_name}")),
            // type -> auto name based on register/field
            "type" => EnumKind::Type(format!("e_{reg_name}_{field_name}")),
            // Any other name -> used as the type name
            name => EnumKind::Type(name.to_owned())
        }
    }

    pub fn name(&self) -> Option<&str> {
        match &self {
            EnumKind::Doc(s) | EnumKind::Type(s) => Some(s),
            _ => None,
        }
    }

    pub fn is_type(&self) -> bool {
        matches!(self,EnumKind::Type(_))
    }

    pub fn is_none(&self) -> bool {
        matches!(self,EnumKind::None)
    }

    pub fn get_type(&self, scope: &str, reg_type: &str, name: &str) -> Option<(String, String)> {
        if let EnumKind::Type(t) = self {
            let mut ts = t.split("::");
            match (ts.next(), ts.next()) {
                (Some(s), Some(n))   => Some((s.to_owned(), n.to_owned())),
                (Some("type"), None) => Some((format!("{scope}_pkg"), format!("e_{reg_type}_{name}"))),
                (Some(n), None)      => Some((format!("{scope}_pkg"), n.to_owned())),
                _ => unreachable!("Invalid enum type {t}"),
            }
        } else {
            None
        }
    }

    pub fn get_def<'a>(&self, defs: &'a EnumDefs) -> Option<&'a EnumDef> {
        self.name()
            .map(|n| defs.find(n))
            .unwrap_or_default()
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum ResetValP {
    Unsigned(u128),
    Signed(i128),
    FloatS(f64),
    FloatU(f64),
    Param(String),
    Enum(String),
}
impl Default for ResetValP {
    fn default() -> Self {
        ResetValP::Unsigned(0)
    }
}

impl Default for &ResetValP {
    fn default() -> Self {
        &ResetValP::Unsigned(0)
    }
}

impl ResetValP {
    //
    pub fn is_signed(&self) -> bool {
        matches!(self,ResetValP::Signed(_) | ResetValP::FloatS(_))
    }

    /// Format the reset value for a `.rif` declaration line (hex for larger unsigned values).
    pub fn to_rif(&self) -> String {
        match self {
            ResetValP::Unsigned(v) => if *v > 9 { format!("0x{v:x}") } else { format!("{v}") },
            ResetValP::Signed(v)   => format!("{v}"),
            ResetValP::FloatU(f) | ResetValP::FloatS(f) => format!("{f}"),
            ResetValP::Param(p)    => format!("${p}"),
            ResetValP::Enum(e)     => e.clone(),
        }
    }

    //
    pub fn compile(&self, signed: bool, nb_frac: isize, params: &ParamValues, enum_def: Option<&EnumDef>) -> Result<ResetVal,String> {
        match self {
            ResetValP::Param(p) => {
                let Some(v) = params.get(p) else {
                    return Err(format!("Unknown parameter {p}"));
                };
                if signed {
                    Ok(ResetVal::Signed(*v as i128))
                } else {
                    Ok(ResetVal::Unsigned(*v as u128))
                }
            }
            ResetValP::Enum(e) => {
                let Some(def) = enum_def else {
                    return Err(format!("Invalid use of enum reset {e} on non-enum field !"));
                };
                let Some(v) = def.get(e) else {
                    return Err(format!("Unknown enum value {e} : expecting {:?}", def.list()));
                };
                Ok(ResetVal::Unsigned(v.value.into()))
            }
            ResetValP::Signed(v)   => Ok(ResetVal::Signed(*v)),
            ResetValP::Unsigned(v) => {
                if signed {Ok(ResetVal::Signed(*v as i128))}
                else {Ok(ResetVal::Unsigned(*v))}
            }
            ResetValP::FloatS(f) |
            ResetValP::FloatU(f) => {
                let v = (*f * (2.0_f64.powi(nb_frac as i32))).round();
                if signed {Ok(ResetVal::Signed(v as i128))}
                else {Ok(ResetVal::Unsigned(v as u128))}
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResetVal {
    Unsigned(u128),
    Signed(i128),
}

impl Default for ResetVal {
    fn default() -> Self {
        ResetVal::Unsigned(0)
    }
}

impl From<ResetValP> for ResetVal {
    fn from(value: ResetValP) -> Self {
        match value {
            ResetValP::Unsigned(v) => ResetVal::Unsigned(v),
            ResetValP::Signed(v) => ResetVal::Signed(v),
            v => unreachable!("Uncompiled reset value {v:?} !"),
        }
    }
}

impl From<&ResetValP> for ResetVal {
    fn from(value: &ResetValP) -> Self {
        match value {
            ResetValP::Unsigned(v) => ResetVal::Unsigned(*v),
            ResetValP::Signed(v) => ResetVal::Signed(*v),
            v => unreachable!("Uncompiled reset value {v:?} !"),
        }
    }
}

impl From<&ResetVal> for ResetValP {
    fn from(value: &ResetVal) -> Self {
        match value {
            ResetVal::Unsigned(v) => ResetValP::Unsigned(*v),
            ResetVal::Signed(v) => ResetValP::Signed(*v),
        }
    }
}

impl ResetVal {
    /// Create a ResetVal::Signed from a u128
    pub fn new_signed(val: u128, width: u8) -> Self {
        let offset = if width < 128 && val >= (1<<(width-1)) {(1<<width) as i128} else {0};
        ResetVal::Signed(val as i128 - offset)
    }

    /// Convert value on w bits (signed or unsigned) to u128
    pub fn to_u128(&self, w: u8) -> u128 {
        match self {
            ResetVal::Unsigned(v) => *v,
            ResetVal::Signed(v) => {
                let mask = if w==0 || w > 127 {u128::MAX} else {(1<<w)-1};
                (*v as u128) & mask
            },
        }
    }

    // Convert value to floating point using a fractoinnal number of bit
    pub fn to_f64(&self, nb_frac: isize) -> f64 {
        let scale = f64::powf(2.0, -nb_frac as f64);
        match self {
            ResetVal::Unsigned(v) => (*v as f64) * scale,
            ResetVal::Signed(v) => (*v as f64) * scale,
        }
    }

    //
    pub fn is_signed(&self) -> bool {
        matches!(self,ResetVal::Signed(_))
    }

}


#[derive(Clone, Debug, PartialEq)]
pub enum Width {
    Value(u8),
    Param(String),
}

impl Width {
    /// Return the value (16b) converting parameter value
    pub fn value(&self, params: &ParamValues) -> Result<u8,String> {
        match self {
            Width::Value(v) => Ok(*v),
            Width::Param(name) =>
                params.get(name)
                    .map(|v| *v as u8)
                    .ok_or_else(|| format!("Unknown parameter {name} in width definition !"))
        }
    }

    /// Return the value (16b) supporting generics as parameter
    pub fn value_g(&self, params: (&ParamValues, &GenericValues)) -> Result<u8,String> {
        match self {
            Width::Value(v) => Ok(*v),
            Width::Param(name) =>
                match params.0.get(name) {
                    Some(v) => Ok(*v as u8),
                    None =>
                        params.1.get(name)
                            .map(|r| r.max as u8)
                            .ok_or_else(|| format!("Unknown parameter/generics {name} in width definition !"))
                }
        }
    }

}

impl Default for Width {
    fn default() -> Self {
        Width::Value(0)
    }
}

impl From<u8> for Width {
    fn from(v: u8) -> Width {
        Width::Value(v)
    }
}
impl From<String> for Width {
    fn from(v: String) -> Width {
        Width::Param(v)
    }
}

impl From<&str> for Width {
    fn from(v: &str) -> Width {
        Width::Param(v.to_owned())
    }
}

impl Display for Width {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Width::Value(v) => write!(f, "{v}"),
            Width::Param(s) => write!(f, "${s}"),
        }
    }
}

/// Addition between two width:
///  output is Value if both are value, otherwise output is Param
impl Add<Width> for Width {
    type Output = Width;

    fn add(self, rhs: Width) -> Self::Output {
        match (self,rhs) {
            (Width::Value(a), Width::Value(b)) => Width::Value(a+b),
            (a, b) => Width::Param(format!("{a}+{b}")),
        }
    }
}

impl Add<&Width> for Width {
    type Output = Width;

    fn add(self, rhs: &Width) -> Self::Output {
        match (self,rhs) {
            (Width::Value(a), Width::Value(b)) => Width::Value(a+b),
            (a, b) => Width::Param(format!("{a}+{b}")),
        }
    }
}

/// Addition between two width:
///  output is Value if both are value, otherwise output is Param
impl Sub<Width> for Width {
    type Output = Width;

    fn sub(self, rhs: Width) -> Self::Output {
        match (self,rhs) {
            (Width::Value(a), Width::Value(b)) => Width::Value(a-b),
            (a, b) => Width::Param(format!("{a}-{b}")),
        }
    }
}

impl Sub<&Width> for Width {
    type Output = Width;

    fn sub(self, rhs: &Width) -> Self::Output {
        match (self,rhs) {
            (Width::Value(a), Width::Value(b)) => Width::Value(a-b),
            (a, b) => Width::Param(format!("{a}-{b}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldPos {
    MsbLsb((Width,Width)),
    LsbSize((Width,Width)),
    Size(Width),
}

impl FieldPos {
    /// Format the position for a `.rif` declaration line: `msb:lsb`, `lsb+:width`, or `Nb`.
    pub fn to_rif(&self) -> String {
        match self {
            FieldPos::MsbLsb((m, l)) => format!("{m}:{l}"),
            FieldPos::LsbSize((l, w)) => format!("{l}+:{w}"),
            FieldPos::Size(w) => format!("{w}b"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LimitValueP {
    None,
    Min(ResetValP),
    Max(ResetValP),
    MinMax(ResetValP, ResetValP),
    List(Vec<ResetValP>),
    Enum,
    External,
}
pub type PairResetVal = (Option<ResetValP>,Option<ResetValP>);
impl From<PairResetVal> for LimitValueP {
	fn from(t: PairResetVal) -> LimitValueP {
		match t {
			(Some(v),None) => LimitValueP::Min(v),
			(None,Some(v)) => LimitValueP::Max(v),
			(Some(v0),Some(v1)) => LimitValueP::MinMax(v0,v1),
			(None,None) => LimitValueP::None,
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub enum LimitValue {
    None,
    Min(ResetVal),
    Max(ResetVal),
    MinMax(ResetVal, ResetVal),
    List(Vec<ResetVal>),
    Enum,
    External,
}


#[derive(Clone, Debug, PartialEq)]
pub struct LimitP {
    pub value: LimitValueP,
    pub bypass: String,
}
impl Default for LimitP {
    fn default() -> Self {
        LimitP {
            value: LimitValueP::None,
            bypass: "".to_owned(),
        }
    }
}

impl LimitP {
    pub fn compile(&self, signed: bool, nb_frac: isize, params: &ParamValues, enum_def: Option<&EnumDef>) -> Result<Limit,String> {
        let value = self.value.compile(signed, nb_frac, params, enum_def)?;
        Ok(Limit{value, bypass:self.bypass.to_owned()})
    }

    /// Serialize the limit to `.rif` source (without the `limit` keyword): `<spec> [bypass]`.
    /// Return `None` when no limit is set.
    pub fn to_rif(&self) -> Option<String> {
        let spec = match &self.value {
            LimitValueP::None => return None,
            LimitValueP::Min(v) => format!("[{}:]", v.to_rif()),
            LimitValueP::Max(v) => format!("[:{}]", v.to_rif()),
            LimitValueP::MinMax(a, b) => format!("[{}:{}]", a.to_rif(), b.to_rif()),
            LimitValueP::List(vs) => format!("{{{}}}", vs.iter().map(|v| v.to_rif()).collect::<Vec<_>>().join(",")),
            LimitValueP::Enum => "enum".to_owned(),
            LimitValueP::External => "external".to_owned(),
        };
        Some(if self.bypass.is_empty() { spec } else { format!("{spec} {}", self.bypass) })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Limit {
    pub value: LimitValue,
    pub bypass: String,
}
impl Default for Limit {
    fn default() -> Self {
        Limit {
            value: LimitValue::None,
            bypass: "".to_owned(),
        }
    }
}

impl Limit {
    pub fn is_none(&self) -> bool {
        self.value == LimitValue::None
    }

    pub fn is_ext(&self) -> bool {
        self.value == LimitValue::External
    }
}

impl LimitValueP {

    /// Create a new limit by setting value to all ResetVal from
    pub fn compile(&self, signed: bool, nb_frac: isize, params: &ParamValues, enum_def: Option<&EnumDef>) -> Result<LimitValue,String> {
        match self {
            LimitValueP::Min(v) => Ok(LimitValue::Min(v.compile(signed, nb_frac, params, enum_def)?)),
            LimitValueP::Max(v) => Ok(LimitValue::Max(v.compile(signed, nb_frac, params, enum_def)?)),
            LimitValueP::MinMax(v0, v1) => Ok(LimitValue::MinMax(
                v0.compile(signed, nb_frac, params, enum_def)?,
                v1.compile(signed, nb_frac, params, enum_def)?)),
            LimitValueP::List(vec) => {
                let mut nv = Vec::with_capacity(vec.len());
                for v in vec {
                    nv.push(v.compile(signed, nb_frac, params, enum_def)?);
                }
                Ok(LimitValue::List(nv))
            }
            LimitValueP::Enum => Ok(LimitValue::Enum),
            LimitValueP::External => Ok(LimitValue::External),
            LimitValueP::None => Ok(LimitValue::None),
            // _ => Ok(self.clone()),
        }
    }

}


#[derive(Clone, Debug, PartialEq)]
pub struct PasswordInfo {
    pub once: Option<ResetValP>,
    pub hold: Option<ResetValP>,
    pub protect: bool,
}

impl PasswordInfo {
    /// Flag when password requires a hold field
    pub fn has_hold(&self) -> bool {
        self.protect || (self.once.is_some() && self.hold.is_some())
    }

    /// Serialize the password settings to `.rif` source (without the `password` keyword): `[once=<val>] [hold=<val>] [protect]`.
    pub fn to_rif(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = &self.once { parts.push(format!("once={}", v.to_rif())); }
        if let Some(v) = &self.hold { parts.push(format!("hold={}", v.to_rif())); }
        if self.protect { parts.push("protect".to_owned()); }
        parts.join(" ")
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum ClkEn {#[default]
    Default,
    None,
    Signal(String),
}

impl ClkEn {
    pub fn is_default(&self) -> bool {
        self==&ClkEn::Default
    }

    // pub fn signal(&self) -> Option<&String> {
    //     match self {
    //         ClkEn::Signal(s) => Some(s),
    //         _ => None
    //     }
    // }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Lock(Option<LogicExpr>);
impl Lock {

    pub fn new(e: LogicExpr) -> Self {
        Lock(Some(e))
    }

    /// Return the lock logical expression
    pub fn expr(&self) -> &Option<LogicExpr> {
        &self.0
    }

    /// Return true if there is a lock
    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    /// Return the lock name if it is part of the structure (i.e. not a path to a different structure)
    pub fn local_field(&self, regname: &str) -> Option<&str> {
        if let Some(e) = &self.0 {
            e.local_field(regname)
        } else {
            None
        }
    }

    /// Return the lock name if it defines an input port (i.e. starts with a .)
    pub fn port_name(&self) -> Option<&str> {
        if let Some(e) = &self.0 {
            e.port_name()
        } else {
            None
        }
    }

}

/// Identifies a field property that lives on its own indented source line
/// Used to track source positions (`SrcInfo::prop_lines`) to support edition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldProp {
    Signed,
    HwAcc,
    NbFrac,
    ArrayPosIncr,
    Lock,
    Limit,
    Counter,
    Password,
    Visibility,
    EnumKind,
    Pulse,
    Interrupt,
}

impl FieldProp {
    /// All managed sub-properties, in the canonical order they are emitted for a new field.
    pub const ALL: [FieldProp; 12] = [
        FieldProp::Signed,
        FieldProp::HwAcc,
        FieldProp::NbFrac,
        FieldProp::ArrayPosIncr,
        FieldProp::Visibility,
        FieldProp::EnumKind,
        FieldProp::Limit,
        FieldProp::Lock,
        FieldProp::Counter,
        FieldProp::Password,
        FieldProp::Pulse,
        FieldProp::Interrupt,
    ];
}

/// Source-tracking metadata attached to a parsed `Field`
/// to allow edition while preserving original style
#[derive(Clone, Debug, Default)]
pub struct FieldSrcInfo {
    /// Line number of the declaration in its source file.
    /// `None` when the field was created programmatically rather than parsed.
    pub decl_line: Option<usize>,
    /// Whether the source declaration line carried an inline `"description"`.
    pub has_inline_desc: bool,
    /// Source line of each managed sub-property present at parse time.
    pub prop_lines: PropLines<FieldProp>,
    /// (start, end) source lines of each tracked description-style block: public/private
    /// `description:`, and, for interrupt-derived fields, `enable`/`mask`/`pending.description:`.
    pub desc_blocks: PropBlocks<DescBlockKind>,
}

impl FieldSrcInfo {
    /// The tracked description-block range for a derived interrupt kind, if any.
    pub fn intr_desc_range(&self, kind: InterruptRegKind) -> Option<(usize, usize)> {
        let key = match kind {
            InterruptRegKind::Enable  => DescBlockKind::IntrEnable,
            InterruptRegKind::Mask    => DescBlockKind::IntrMask,
            InterruptRegKind::Pending => DescBlockKind::IntrPending,
            _ => return None,
        };
        self.desc_blocks.get(&key).copied()
    }
}

/// Source position never affects equality: see `DeclLine`, which documents the same rule
/// for `EnumDef`/`EnumEntry`.
impl PartialEq for FieldSrcInfo {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// Field name
    pub name: String,
    /// Field position/Size inside a register
    pub pos: FieldPos,
    /// Size of the array
    pub array: Width,
    /// Field position increment when defined as an array
    pub array_pos_incr: u8,
    /// Reset value (one per array element)
    pub reset: Vec<ResetValP>,
    /// True when field is signed
    pub signed: bool,
    /// Description
    pub description: Description,
    /// Enumerate type
    pub enum_kind: EnumKind,
    /// Hardware access modifier
    pub hw_kind: Vec<FieldHwKind>,
    /// Software access kind
    pub sw_kind: FieldSwKind,
    /// Partial definition (bus/array)
    pub partial: (Option<u16>, u16),
    /// Simplified hardware access (NA/Read/Write)
    pub hw_acc: Access,
    /// Optional clock signal: auto-selected if not present
    pub clk: Option<String>,
    /// Optional clock enable signal
    pub clk_en: ClkEn,
    /// Optional clear signal
    pub clear: Option<LogicExpr>,
    /// Optional lock signal to prevent write access
    pub lock: Lock,
    /// Field visibility
    pub visibility: VisibilityRaw,
    /// Optional description for interrupt derived register (enable/mask/pending)
    pub intr_desc: Option<InterruptDesc>,
    /// Interrupt trigger/clear override. Empty when field just inherits the owning register setting
    pub intr_ovr: InterruptInfoField,
    /// Optional limits on the value which can be writen
    pub limit: LimitP,
    /// Number of fractional bits
    pub nb_frac: isize,
    /// Indicates the register instance is controlled by a parameter
    pub optional: String,
    /// Extra Info
    pub info: HashMap<String,String>,
    /// Source-tracking metadata for round-tripping edits back to the `.rif` file.
    pub src: FieldSrcInfo,
}

impl Default for Field {
    fn default() -> Self {
        Field {
            name: "".to_owned(),
            description: "".into(),
            pos: FieldPos::Size(Width::Value(1)),
            array: Width::Value(0),
            array_pos_incr: 0,
            reset: vec![ResetValP::Unsigned(0)],
            signed: false,
            partial: (None, 0),
            enum_kind: EnumKind::None,
            hw_kind: Vec::new(),
            sw_kind: FieldSwKind::default(),
            hw_acc: Access::RO,
            clk: None,
            clk_en: ClkEn::Default,
            clear: None,
            lock: Lock(None),
            visibility: VisibilityRaw::Full,
            intr_desc: None,
            intr_ovr: InterruptInfoField::default(),
            nb_frac : 0,
            limit: LimitP::default(),
            info: HashMap::new(),
            optional: "".to_owned(),
            src: FieldSrcInfo::default(),
        }
    }
}

impl Field {
    pub fn new<S1, S2>(
        name: S1,
        reset: Vec<ResetValP>,
        pos: FieldPos,
        sw_kind: Option<FieldSwKind>,
        array: Option<Width>,
        desc: S2,
    ) -> Self
    where
        S1: Into<String>,
        S2: Into<Description>,
    {
        // Presence of a reset value determined a default hardware access and software kind
        let (hw_acc, sw_kind) =
            if let Some(kind) = sw_kind {
                (match kind {
                    FieldSwKind::ReadOnly |
                    FieldSwKind::ReadClr  |
                    FieldSwKind::W1Clr    |
                    FieldSwKind::W0Clr    |
                    FieldSwKind::W1Set    => Access::WO,
                    FieldSwKind::ReadWrite  |
                    FieldSwKind::WriteOnly  |
                    FieldSwKind::W1Tgl      |
                    FieldSwKind::W1Pulse(_,_) => Access::RO,
                    FieldSwKind::Password(_) => Access::NA,
                }, kind)
            } else if reset.is_empty() {
                (Access::WO, FieldSwKind::ReadOnly)
            } else {
                (Access::RO, FieldSwKind::ReadWrite)
            };
        let signed = reset.first().map(|r| r.is_signed()).unwrap_or(false);
        Field {
            name: name.into(),
            description: desc.into(),
            pos,
            signed,
            array: array.unwrap_or(Width::Value(0)),
            array_pos_incr: 0,
            reset: if reset.is_empty() {vec![ResetValP::Unsigned(0)]} else {reset},
            sw_kind,
            hw_acc,
            ..Default::default()
        }
    }

    // Only allow changing hardware kind when basic
    pub fn set_hw_kind(&mut self, kind: FieldHwKind) -> Result<(), RifError> {
        if let Some(kind_prev) = self.hw_kind.last() {
            // Check kind compatibility
            let ok = match kind {
                FieldHwKind::Set(_) |
                FieldHwKind::Toggle(_) |
                FieldHwKind::Clear(_) |
                FieldHwKind::WriteEn(_) |
                FieldHwKind::WriteEnL(_) => matches!(kind_prev,
                    FieldHwKind::Set(_) | FieldHwKind::Toggle(_) | FieldHwKind::Clear(_) |
                    FieldHwKind::WriteEn(_) | FieldHwKind::WriteEnL(_)),
                // Other kinds are exclusive
                _ => false,
            };
            if !ok {
                return Err(RifError::field_kind(&kind, &self.hw_kind));
            }
        }
        self.hw_kind.push(kind);
        Ok(())

    }

    /// Hardware access a field gets implicitly from its software kind
    pub fn default_hw_acc(&self) -> Access {
        match self.sw_kind {
            // Write-only when software access is read or clear
            FieldSwKind::ReadOnly |
            FieldSwKind::ReadClr  |
            FieldSwKind::W1Clr    |
            FieldSwKind::W0Clr    |
            FieldSwKind::W1Set    => Access::WO,
            // Write-only when software access can write a value
            FieldSwKind::ReadWrite     |
            FieldSwKind::WriteOnly     |
            FieldSwKind::W1Tgl         |
            FieldSwKind::W1Pulse(_, _) => Access::RO,
            // Not-available for password field
            FieldSwKind::Password(_) => Access::NA,
        }
    }

    pub fn set_hw_acc(&mut self, acc: Access) {
        // Handle case when trying to set hardware access as read-only
        // when there is already a hardware write access defined
        self.hw_acc = if !self.hw_kind.is_empty() && acc==Access::RO {
            Access::RW
        } else {
            acc
        };
    }

    pub fn set_sw_kind(&mut self, kind: FieldSwKind) -> Result<(), RifError> {
        match kind {
            FieldSwKind::W1Pulse(_,_) => {
                self.hw_acc = Access::RO;
                if !self.hw_kind.is_empty() {
                    return Err(RifError::field_kind(&kind, &self.hw_kind));
                }
            }
            // Reset value for password field is 1 since it corresponds to the locked signal
            FieldSwKind::Password(_) => {
                for r in self.reset.iter_mut() {
                    *r = ResetValP::Unsigned(1);
                };
            }
            _ => {}
        }
        self.sw_kind = kind;
        Ok(())
    }

    // Change all unsigned value to signed
    pub fn set_signed(&mut self) {
        self.signed = true;
        for r in self.reset.iter_mut() {
            if let ResetValP::Unsigned(v) = r {
                *r = ResetValP::Signed(*v as i128);
            }
        };
    }

    /// Flag when a field is split on multiple register
    pub fn is_partial(&self) -> bool {
        self.partial.0.is_some()
    }

    /// Update the SW/HW kind access to match the interrupt settings
    pub fn set_intr(&mut self, value: InterruptInfoField) {
        if self.hw_kind.is_empty() {
            self.hw_kind.push(FieldHwKind::Interrupt(value.trigger.unwrap_or_default()));
        } else if let Some(trigger) = value.trigger {
            *self.hw_kind.first_mut().expect("Interrupt field should be part of interrupt register") = FieldHwKind::Interrupt(trigger);
        }
        match value.clear {
            Some(InterruptClr::Read)   => self.sw_kind = FieldSwKind::ReadClr,
            Some(InterruptClr::Write0) => self.sw_kind = FieldSwKind::W0Clr,
            Some(InterruptClr::Write1) => self.sw_kind = FieldSwKind::W1Clr,
            Some(InterruptClr::Hw)     => self.hw_kind.push(FieldHwKind::Clear(None)),
            None => {}
        };
    }

    /// Override this field's interrupt trigger/clear.
    pub fn set_intr_ovr(&mut self, value: InterruptInfoField, reg_default: InterruptInfoField) {
        self.intr_ovr = value.clone();
        self.set_intr(InterruptInfoField {
            trigger: value.trigger.or(reg_default.trigger),
            clear: value.clear.or(reg_default.clear),
        });
    }

    /// Field width
    pub fn width(&self, params: (&ParamValues, &GenericValues)) -> Result<u8, String> {
        match &self.pos {
            FieldPos::MsbLsb((m,l)) => Ok(m.value(params.0)? - l.value(params.0)? + 1),
            FieldPos::LsbSize((_,w)) => w.value_g(params),
            FieldPos::Size(w) => w.value_g(params),
        }
    }

    /// Return an optional HwKind when unset and write access limited to clear or set
    pub fn get_auto_hw_kind(&self, params:  (&ParamValues, &GenericValues)) -> Option<FieldHwKind> {
        if self.hw_kind.is_empty() && self.hw_acc.is_writable() && self.sw_kind!=FieldSwKind::ReadOnly {
            let w = self.width(params).unwrap_or(2); // In case of parametr not found just return large enough value to be considered as a bus. This will anyway be catch at another level
            if self.sw_kind.is_set() {
                Some(FieldHwKind::Clear(None))
            } else if w > 1 || (w==1 && !self.sw_kind.is_clr()) {
                Some(FieldHwKind::WriteEn(None))
            } else if self.sw_kind.is_clr() {
                Some(FieldHwKind::Set(None))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Set visibility to hidden
    pub fn hidden(&mut self) {
        self.visibility = VisibilityRaw::Hidden;
    }

    /// Set visibility to reserved
    pub fn reserved(&mut self) {
        self.visibility = VisibilityRaw::Reserved;
    }

    /// Set visibility to reserved
    pub fn disabled(&mut self, expr: ExprTokens) {
        self.visibility = VisibilityRaw::Disabled(expr);
    }

    /// Update an interrupt description
    pub fn desc_intr_updt(&mut self, cntxt: &Context, desc: &str, is_private: bool) {
        if self.intr_desc.is_none() {
            self.intr_desc = Some(InterruptDesc::default());
        }
        let d = match cntxt {
            Context::DescIntrEnable => &mut self.intr_desc.as_mut().unwrap().enable,
            Context::DescIntrMask => &mut self.intr_desc.as_mut().unwrap().mask,
            Context::DescIntrPending => &mut self.intr_desc.as_mut().unwrap().pending,
            _ => unreachable!(),
        };
        d.updt(desc, is_private);
    }

    /// Return the lock name if it is part of the structure (i.e. not a path to a different structure)
    pub fn get_local_lock(&self, regname: &str) -> Option<&str> {
        self.lock.local_field(regname)
    }

    /// Serialize the field's *declaration line* back to `.rif` syntax, prefixed with `indent`.
    pub fn fmt_decl(&self, indent: &str) -> String {
        let mut s = String::with_capacity(indent.len() + self.name.len() + 24);
        s.push_str(indent);
        s.push_str("- ");
        s.push_str(&self.name);
        if let Width::Value(n) = &self.array
            && *n > 0
        {
            s.push_str(&format!("[{n}]"));
        }
        // Reset value (default 0 is still written to keep the line unambiguous when edited).
        s.push_str(" = ");
        if self.reset.len() > 1 {
            s.push_str(&format!("{{{}}}", self.reset.iter().map(|r| r.to_rif()).collect::<Vec<_>>().join(",")));
        } else {
            s.push_str(&self.reset.first().unwrap_or(&ResetValP::Unsigned(0)).to_rif());
        }
        // Position
        s.push(' ');
        s.push_str(&self.pos.to_rif());
        // Software access kind (omitted for the implicit read/write default)
        if let Some(tok) = self.sw_kind.inline_token() {
            s.push(' ');
            s.push_str(tok);
        }
        // Inline short description, only when the source originally had one
        if self.src.has_inline_desc {
            let short = self.description.get_short(false);
            if !short.is_empty() {
                s.push_str(" \"");
                s.push_str(&short);
                s.push('"');
            }
        }
        s
    }

    /// Serialize a managed sub-property as its canonical `.rif` line, prefixed with `indent`.
    /// Returns `None` when the property is inactive / at its default.
    /// Currently not supporting (returning None) for complex `lock` expression,and `disabled` visibility expression).
    pub fn fmt_prop(&self, prop: FieldProp, indent: &str) -> Option<String> {
        let body = match prop {
            FieldProp::Signed => if self.signed { "signed".to_owned() } else { return None },
            FieldProp::HwAcc => if self.hw_acc == self.default_hw_acc() {
                return None;
            } else {
                format!("hw {}", self.hw_acc.to_string().to_lowercase())
            },
            FieldProp::NbFrac => if self.nb_frac != 0 { format!("nbfrac {}", self.nb_frac) } else { return None },
            FieldProp::ArrayPosIncr => if self.array_pos_incr != 0 { format!("arrayPosIncr {}", self.array_pos_incr) } else { return None },
            FieldProp::Visibility => match &self.visibility {
                VisibilityRaw::Hidden => "hidden".to_owned(),
                VisibilityRaw::Reserved => "reserved".to_owned(),
                // `disabled` carries an expression that is not serialized yet: leave it alone.
                VisibilityRaw::Full | VisibilityRaw::Disabled(_) => return None,
            },
            FieldProp::EnumKind => match &self.enum_kind {
                EnumKind::None => return None,
                EnumKind::Doc(_) => "enum".to_owned(),
                EnumKind::Type(name) => format!("enum {name}"),
            },
            FieldProp::Limit => format!("limit {}", self.limit.to_rif()?),
            FieldProp::Lock => format!("lock {}", self.lock.expr().as_ref()?.to_rif()?),
            FieldProp::Counter => {
                let c = self.hw_kind.iter().find_map(|k| match k {
                    FieldHwKind::Counter(c) => Some(c),
                    _ => None,
                })?;
                format!("counter {}", c.to_rif())
            }
            FieldProp::Password => match &self.sw_kind {
                FieldSwKind::Password(info) => {
                    let args = info.to_rif();
                    if args.is_empty() { "password".to_owned() } else { format!("password {args}") }
                }
                _ => return None,
            },
            FieldProp::Pulse => match &self.sw_kind {
                FieldSwKind::W1Pulse(_, _) if self.sw_kind.inline_token().is_some() => return None,
                FieldSwKind::W1Pulse(true, _) => "pulse reg".to_owned(),
                FieldSwKind::W1Pulse(false, _) => "pulse comb".to_owned(),
                _ => return None,
            },
            FieldProp::Interrupt => {
                if self.intr_ovr.trigger.is_none() && self.intr_ovr.clear.is_none() {
                    return None;
                }
                let mut body = "interrupt".to_owned();
                if let Some(trigger) = self.intr_ovr.trigger {
                    body.push(' ');
                    body.push_str(trigger.to_rif());
                }
                if let Some(clear) = self.intr_ovr.clear {
                    body.push(' ');
                    body.push_str(clear.to_rif());
                }
                body
            }
        };
        Some(format!("{indent}{body}"))
    }

    /// Print all properties lines, in canonical order, prefixed with `indent`.
    pub fn fmt_prop_all(&self, indent: &str) -> Vec<String> {
        FieldProp::ALL.iter().filter_map(|&p| self.fmt_prop(p, indent)).collect()
    }

    /// Serialize the field's public `description:` block.
    pub fn fmt_desc_block(&self, indent: &str) -> Option<Vec<String>> {
        let rest = self.description.get_split(true).1?;
        let body_indent = format!("{indent}  ");
        let mut lines = vec![format!("{indent}description:")];
        lines.extend(rest.split('\n').map(|l| format!("{body_indent}{l}")));
        Some(lines)
    }

    /// Serialize this field's `{enable,mask,pending}.description:` block
    pub fn fmt_intr_desc_block(&self, kind: InterruptRegKind, indent: &str) -> Option<Vec<String>> {
        let intr_desc = self.intr_desc.as_ref()?;
        let desc = match kind {
            InterruptRegKind::Enable  => &intr_desc.enable,
            InterruptRegKind::Mask    => &intr_desc.mask,
            InterruptRegKind::Pending => &intr_desc.pending,
            _ => return None,
        };
        if desc.is_empty(true) {
            return None;
        }
        let keyword = match kind {
            InterruptRegKind::Enable  => "enable",
            InterruptRegKind::Mask    => "mask",
            InterruptRegKind::Pending => "pending",
            _ => unreachable!("checked above"),
        };
        let body_indent = format!("{indent}  ");
        let mut lines = vec![format!("{indent}{keyword}.description:")];
        lines.extend(desc.get(true).split('\n').map(|l| format!("{body_indent}{l}")));
        Some(lines)
    }

    /// Mutable access to this field's description override for a derived interrupt kind,
    pub fn intr_desc_mut(&mut self, kind: InterruptRegKind) -> &mut Description {
        let intr_desc = self.intr_desc.get_or_insert_with(InterruptDesc::default);
        match kind {
            InterruptRegKind::Enable  => &mut intr_desc.enable,
            InterruptRegKind::Mask    => &mut intr_desc.mask,
            InterruptRegKind::Pending => &mut intr_desc.pending,
            _ => unreachable!("Only kind possible Enable/Mask/Pending"),
        }
    }

}

