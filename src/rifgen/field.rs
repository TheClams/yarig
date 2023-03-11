use std::{collections::HashMap, fmt::Display, ops::{Add, Sub}};

use crate::{error::RifError, parser::parser_expr::ParamValues};

use super::{
    Context, Description, InterruptClr, InterruptDesc, InterruptInfoField, InterruptTrigger,
};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PulseLogic {
    Comb,
    Reg,
    ResyncSimple,
    ResyncAsync,
}

/// Generic access
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Access {
	/// Read/Write
    RW,
    /// Read-Only
    RO,
    /// Write-Only
    WO,
    /// Not Available
    NA,
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


#[derive(Clone, Debug, PartialEq)]
pub struct EnumEntry {
    pub name: String,
    pub value: u8,
    pub description: Description,
}
impl EnumEntry {
    #[allow(dead_code)]
    pub fn new<S1, S2>(name: S1, value: u8, description: S2) -> Self
    where
        S1: Into<String>,
        S2: Into<Description>,
    {
        EnumEntry {
            name: name.into(),
            description: description.into(),
            value,
        }
    }
}
// pub type EnumDef = Vec<EnumEntry>;
#[derive(Clone, Debug)]
pub struct EnumDef {
    pub name: String,
    pub description: String,
    pub values: Vec<EnumEntry>,
}
impl EnumDef {
    pub fn new(name: String, description: String) -> Self {
        EnumDef {name, description, values: Vec::with_capacity(4)}
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn iter(&self) -> impl Iterator<Item=&EnumEntry> {
        self.values.iter()
    }

    pub fn is_local_type(&self) -> bool {
        !self.name.contains(':')
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CounterKind {
    Up,
    Down,
    UpDown,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CounterInfo {
    pub kind: CounterKind,
    pub incr_val: u8,
    pub decr_val: u8,
    pub sat: bool,
    pub clr: bool,
    pub event: bool,
}

impl CounterInfo {
    pub fn is_up(&self) -> bool {
        matches!(self.kind, CounterKind::Up | CounterKind::UpDown)
    }
    pub fn is_down(&self) -> bool {
        matches!(self.kind, CounterKind::Down | CounterKind::UpDown)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum Visibility {#[default]
    Full,
    Hidden,
    Reserved,
    Disabled,
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
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum FieldHwKind {#[default]
    ReadOnly,
    Set(Option<String>),
    Toggle(Option<String>),
    Clear(Option<String>),
    WriteEn(Option<String>),
    WriteEnL(Option<String>),
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
    pub fn get_signal(&self) -> &Option<String> {
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

    pub fn is_pulse_comb(&self) -> bool {
        matches!(self, FieldSwKind::W1Pulse(false,_))
    }

    pub fn is_wo(&self) -> bool {
        matches!(self, FieldSwKind::WriteOnly | FieldSwKind::W1Pulse(_,true) | FieldSwKind::Password(_))
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
            "" => {
                EnumKind::Doc(format!(
                    "doc:{}_{}",
                    reg_name,
                    field_name
                ))
            }
            // type -> auto name based on register/field
            "type" => {
                EnumKind::Type(format!(
                    "e_{}_{}",
                    reg_name,
                    field_name
                ))
            }
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
    #[allow(dead_code)]
    pub fn is_type(&self) -> bool {
        matches!(self,EnumKind::Type(_))
    }
}


#[derive(Clone, Debug, PartialEq)]
pub enum ResetVal {
    Unsigned(u128),
    Signed(i128),
    Param(String),
}
impl Default for ResetVal {
    fn default() -> Self {
        ResetVal::Unsigned(0)
    }
}

impl Default for &ResetVal {
    fn default() -> Self {
        &ResetVal::Unsigned(0)
    }
}

impl ResetVal {
    // Suppose used only on compiled value
    pub fn to_u128(&self, w: u8) -> u128 {
        match self {
            ResetVal::Unsigned(v) => *v,
            ResetVal::Signed(v) => (*v as u128) & ((1<<w)-1),
            ResetVal::Param(p) => unreachable!("to_u128 cannot be used on uncompiled values: {:?}",p),
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
    pub fn value(&self, params: &ParamValues) -> u8 {
        match self {
            Width::Value(v) => *v,
            Width::Param(name) => *params.get(name).unwrap() as u8,
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
            Width::Value(v) => write!(f, "{}",v),
            Width::Param(s) => write!(f, "{}",s),
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
            (a, b) => Width::Param(format!("{}+{}",a,b)),
        }
    }
}

impl Add<&Width> for Width {
    type Output = Width;

    fn add(self, rhs: &Width) -> Self::Output {
        match (self,rhs) {
            (Width::Value(a), Width::Value(b)) => Width::Value(a+b),
            (a, b) => Width::Param(format!("{}+{}",a,b)),
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
            (a, b) => Width::Param(format!("{}-{}",a,b)),
        }
    }
}

impl Sub<&Width> for Width {
    type Output = Width;

    fn sub(self, rhs: &Width) -> Self::Output {
        match (self,rhs) {
            (Width::Value(a), Width::Value(b)) => Width::Value(a-b),
            (a, b) => Width::Param(format!("{}-{}",a,b)),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum FieldPos {
    MsbLsb((Width,Width)),
    LsbSize((Width,Width)),
    Size(Width),
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum LimitValue {
    None,
    Min(ResetVal),
    Max(ResetVal),
    MinMax(ResetVal, ResetVal),
    List(Vec<ResetVal>),
    Enum,
}
pub type PairResetVal = (Option<ResetVal>,Option<ResetVal>);
impl From<PairResetVal> for LimitValue {
	fn from(t: PairResetVal) -> LimitValue {
		match t {
			(Some(v),None) => LimitValue::Min(v),
			(None,Some(v)) => LimitValue::Max(v),
			(Some(v0),Some(v1)) => LimitValue::MinMax(v0,v1),
			(None,None) => LimitValue::None,
		}
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
}



#[derive(Clone, Debug, PartialEq)]
pub struct PasswordInfo {
    pub once: Option<ResetVal>,
    pub hold: Option<ResetVal>,
    pub protect: bool,
}

impl PasswordInfo {
    /// Flag when password requires a hold field
    pub fn has_hold(&self) -> bool {
        self.protect || (self.once.is_some() && self.hold.is_some())
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
pub struct Lock(Option<String>);
impl Lock {

    pub fn new(name: String) -> Self {
        Lock(Some(name))
    }

    /// Return the lock name if it is part of the structure (i.e. not a path to a different structure)
    pub fn name(&self) -> &Option<String> {
        &self.0
    }

    /// Return true if there is a lock
    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }

    /// Return the lock name if it is part of the structure (i.e. not a path to a different structure)
    pub fn local_name(&self) -> Option<&String> {
        self.0.as_ref().filter(|lock| !lock.contains('.'))
    }

    /// Return the lock name if it defines an input port (i.e. starts with a .)
    pub fn port_name(&self) -> Option<&str> {
        if let Some(lock) = &self.0 {
            if let Some(lock) = lock.strip_prefix('.') {
                Some(lock)
            } else {
                None
            }
        } else {
            None
        }
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
    pub reset: Vec<ResetVal>,
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
    pub clear: Option<String>,
    /// Optional lock signal to prevent write access
    pub lock: Lock,
    /// Field visibility
    pub visibility: Visibility,
    /// Optional description for interrupt derived register (enable/mask/pending)
    pub intr_desc: Option<InterruptDesc>,
    /// Optional limits on the value which can be writen
    pub limit: Limit,
    /// Indicates the register instance is controlled by a parameter
    pub optional: String,
    /// Extra Info
    pub info: HashMap<String,String>,
}

impl Default for Field {
    fn default() -> Self {
        Field {
            name: "".to_owned(),
            description: "".into(),
            pos: FieldPos::Size(Width::Value(1)),
            array: Width::Value(0),
            array_pos_incr: 0,
            reset: vec![ResetVal::Unsigned(0)],
            partial: (None, 0),
            enum_kind: EnumKind::None,
            hw_kind: Vec::new(),
            sw_kind: FieldSwKind::default(),
            hw_acc: Access::RO,
            clk: None,
            clk_en: ClkEn::Default,
            clear: None,
            lock: Lock(None),
            visibility: Visibility::Full,
            intr_desc: None,
            limit: Limit::default(),
            info: HashMap::new(),
            optional: "".to_owned(),
        }
    }
}

impl Field {
    pub fn new<S1, S2>(
        name: S1,
        reset: Vec<ResetVal>,
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
        Field {
            name: name.into(),
            description: desc.into(),
            pos,
            array: array.unwrap_or(Width::Value(0)),
            array_pos_incr: 0,
            reset: if reset.is_empty() {vec![ResetVal::Unsigned(0)]} else {reset},
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
                return
                    Err(RifError {
                        kind: crate::error::RifErrorKind::FieldKind,
                        line_num: 0, // TODO
                        txt: format!("{:?} and {:?}", self.hw_kind, kind),
                    });
            }
        }
        self.hw_kind.push(kind);
        Ok(())

    }

    pub fn set_sw_kind(&mut self, kind: FieldSwKind) -> Result<(), RifError> {
        match kind {
            FieldSwKind::W1Pulse(_,_) => {
                self.hw_acc = Access::RO;
                if !self.hw_kind.is_empty() {
                    return Err(RifError {
                        kind: crate::error::RifErrorKind::FieldKind,
                        line_num: 0, // TODO
                        txt: format!("{:?} and {:?}", self.hw_kind, kind),
                    });
                }
            }
            // Reset value for password field is 1 since it corresponds to the locked signal
            FieldSwKind::Password(_) => {
                for r in self.reset.iter_mut() {
                    *r = ResetVal::Unsigned(1);
                };
            }
            _ => {}
        }
        self.sw_kind = kind;
        Ok(())
    }

    // Change all unsigned value to signed
    pub fn signed(&mut self) {
        for r in self.reset.iter_mut() {
            if let ResetVal::Unsigned(v) = r {
                *r = ResetVal::Signed(*v as i128);
            }
        };
    }

    /// Set interrupt settings
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

    /// Field width
    pub fn width(&self, params: &ParamValues) -> u8 {
        match &self.pos {
            FieldPos::MsbLsb((m,l)) => m.value(params) - l.value(params) + 1,
            FieldPos::LsbSize((_,w)) => w.value(params),
            FieldPos::Size(w) => w.value(params),
        }
    }

    /// Return an optional HwKind when unset and write access limited to clear or set
    pub fn get_auto_hw_kind(&self, params: &ParamValues) -> Option<FieldHwKind> {
        if self.hw_kind.is_empty() {
            if self.sw_kind.is_clr() {
                if self.width(params) > 1 {
                    Some(FieldHwKind::WriteEn(None))
                } else {
                    Some(FieldHwKind::Set(None))
                }
            } else if self.sw_kind.is_set() {
                Some(FieldHwKind::Clear(None))
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Set visibility to hidden
    pub fn hidden(&mut self) {
        self.visibility = Visibility::Hidden;
    }

    /// Set visibility to reserved
    pub fn reserved(&mut self) {
        self.visibility = Visibility::Reserved;
    }

    /// Update an interrupt description
    pub fn desc_intr_updt(&mut self, cntxt: &Context, desc: &str) {
        if self.intr_desc.is_none() {
            self.intr_desc = Some(InterruptDesc::default());
        }
        let d = match cntxt {
            Context::DescIntrEnable => &mut self.intr_desc.as_mut().unwrap().enable,
            Context::DescIntrMask => &mut self.intr_desc.as_mut().unwrap().mask,
            Context::DescIntrPending => &mut self.intr_desc.as_mut().unwrap().pending,
            _ => unreachable!(),
        };
        d.updt(desc);
    }

    /// Return the lock name if it is part of the structure (i.e. not a path to a different structure)
    pub fn get_local_lock(&self) -> Option<&String> {
        self.lock.local_name()
    }

}
