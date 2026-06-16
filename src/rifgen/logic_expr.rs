use crate::comp::{comp_inst::RifFieldInst, reg_impl::MissingFieldInfo};

use super::ResetVal;


#[derive(Clone, Debug, PartialEq)]
pub enum SignalRange {
    /// Fixed range defined by the pair (msb,lsb)
    Fixed((u8,u8)),
    /// Generic range defined by a LSB and parameterized width
    GenericWidth((String,u8)),
    /// Generic range defined by a MSB and parameterized LSB
    GenericLsb((u8,String)),
}

impl SignalRange {
    /// Create a fixed range
    pub fn new(lsb: u8, msb: u8) -> Self {
        SignalRange::Fixed((msb,lsb))
    }

    /// Create a fixed single bit range
    pub fn new_bit(lsb: u8) -> Self {
        SignalRange::Fixed((lsb,lsb))
    }

    /// Create a range where width is generic
    pub fn new_gen(name: String, lsb: u8) -> Self {
        SignalRange::GenericWidth((name,lsb))
    }

    /// Check if the range is a single bit
    pub fn is_bit(&self) -> bool {
        match self {
            SignalRange::Fixed((msb,lsb)) => lsb==msb,
            SignalRange::GenericWidth(_) => false,
            SignalRange::GenericLsb(_) => false,
        }
    }

    /// Return LSB of the range
    pub fn lsb(&self) -> u8 {
        match self {
            SignalRange::Fixed((_,lsb)) => *lsb,
            SignalRange::GenericWidth((_,lsb)) => *lsb,
            SignalRange::GenericLsb(_) => 0, // Might need to be reviwed if this method is used elsewhere than logic_expr_parser
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
    /// Cast a value from enumerated type to bit vector
    CastFrom(String, u8, Box<LogicExpr>),
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
    /// Greater Than between two logic expression
    Gt(Box<LogicExpr>,Box<LogicExpr>),
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

/// Implement conversion from a couple value,max
impl From<(u16,u16)> for LogicExpr {
    fn from(value: (u16,u16)) -> Self {
        let nb_bits = (u16::BITS - (value.1-1).leading_zeros()).max(1);
        LogicExpr::ValueU(
            value.0 as u128,
            nb_bits as usize
        )
    }
}

impl LogicExpr {
    /// Create a Value (Signed/Unsigned) from a u128 and a field definition (for signed and width)
    pub fn value(v: u128, info: &RifFieldInst) -> LogicExpr {
        let w = info.width.value() as usize;
        if info.is_signed() {
            LogicExpr::ValueI(v as i128, w)
        } else {
            LogicExpr::ValueU(v, w)
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
    pub fn gt(lhs: LogicExpr, rhs: LogicExpr) -> LogicExpr {
        LogicExpr::Gt(Box::new(lhs), Box::new(rhs))
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

    pub fn is_or(&self) -> bool {
        matches!(*self, LogicExpr::Or(_))
    }

    pub fn has_vec(&self) -> bool {
        matches!(*self, LogicExpr::Ite(_,_) | LogicExpr::Or(_) | LogicExpr::And(_) )
    }

    pub fn is_comp(&self) -> bool {
        matches!(*self, LogicExpr::Gte(_,_) | LogicExpr::Gt(_,_) | LogicExpr::Lte(_,_) | LogicExpr::Lt(_,_) | LogicExpr::Neq(_,_) | LogicExpr::Eq(_,_))
    }

    pub fn has_comp(&self) -> bool {
        match self {
            LogicExpr::Eq(_,_)  |
            LogicExpr::Neq(_,_)  |
            LogicExpr::Gte(_,_)  |
            LogicExpr::Gt(_,_)   |
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
            LogicExpr::CastFrom(_,_,_) |
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
            LogicExpr::Gt(_,_)  |
            LogicExpr::Lte(_,_)  |
            LogicExpr::Lt(_,_)   => 2,
        }
    }

    /// Return the lock name if it is part of the structure (i.e. not a path to a different structure)
    pub fn local_field(&self, regname: &str) -> Option<&str> {
        if let LogicExpr::Id(id) = self {
            if ["this", "self", regname].contains(&id.name.as_str()) {
                id.field.as_deref()
            } else if id.field.is_none() {
                Some(&id.name)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Return the name if the expression is a simple ID in the form ".name"
    pub fn port_name(&self) -> Option<&str> {
        if let LogicExpr::Id(id) = self {
            if id.name.is_empty() {
                id.field.as_deref()
            } else {
                None
            }
        } else {
            None
        }
    }

}