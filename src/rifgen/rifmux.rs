use std::{collections::{BTreeMap, HashMap}, str::FromStr};

use crate::parser::{parser_expr::{ExprTokens, ParamValues}, suffix_info};

use super::{order_dict::OrderDict, AddressKind, ClockingInfo, Description, Interface};

#[derive(Clone, Debug)]
pub struct Rifmux {
    /// Type name
    pub name: String,
    /// Address bus width
    pub addr_width: u8,
    /// Data bus width
    pub data_width: u8,
    /// Software clocking defintion (clock, reset, enable)
    pub sw_clocking: ClockingInfo,
    /// Hardware Interface
    pub interface: Interface,
    /// Items inside Rifmux (Rif or other rifmux)
    pub items: Vec<RifmuxItem>,
    /// Parameter definition
    pub parameters: OrderDict<String,ExprTokens>,
    /// Top description
    pub description: Description,
    /// List of component group
    pub groups: Vec<RifmuxGroup>,
    /// Optional hardware top level
    pub top: Option<RifmuxTop>,
    /// Extra custom informations
    pub info: HashMap<String,String>,
}

impl Rifmux {
    pub fn new<S>(name: S) -> Self where S: Into<String> {
        Rifmux {
            name: name.into(),
            addr_width: 16,
            data_width: 32,
            interface: Interface::Default,
            sw_clocking: ClockingInfo::default(),
            items: vec![],
            groups: vec![],
            description: "".into(),
            parameters: OrderDict::new(),
            info: HashMap::new(),
            top: None,
        }
    }

    pub fn add_info(&mut self, key_val:(&str, &str)) {
        self.info.insert(key_val.0.to_owned(), key_val.1.to_owned());
    }

    pub fn add_param(&mut self, key: &str, expr: ExprTokens) {
        self.parameters.insert(key.to_owned(), expr);
    }

    pub fn add_top_suffix(&mut self, key: &str, val: &str) {
        if let Some(ref mut top) = self.top {
            top.prefixes.insert(key.to_owned(),val.to_owned());
        }
    }

}


#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum RifType {Rif(String), Ext(u8)}
/// Tuple from parser
/// Values are: instance name, array size, type name, group name, addressing scheme and address
pub type RifmuxItemTuple<'a> = (&'a str, RifType, Option<(AddressKind, AddressOffset)>, Option<&'a str>);

#[derive(Clone, Debug, PartialEq, Default)]
pub struct SuffixInfo {
    pub name: String,
    pub alt_pos: bool,
    pub pkg: bool,
}

impl SuffixInfo {
    pub fn new(name: String, alt_pos: bool, pkg: bool) -> Self {
        SuffixInfo {name, alt_pos, pkg}
    }
}

impl FromStr for SuffixInfo {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        suffix_info(s).map_err(|_| format!("Unable to parse {s} as a SuffixInfo. Format is suffix_name(alt,pkg) with alt and pkg optional."))
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Item inside a Rifmux
pub struct RifmuxItem {
    /// Name of the RIF instance
    pub name: String,
    /// Name of the group
    pub group: String,
    /// Name of the register type
    pub rif_type: RifType,
    /// Addressing scheme used: absolute or relative
    pub addr_kind: AddressKind,
    /// Address of the instance
    pub addr: AddressOffset,
    /// Description
    pub description: Description,
    /// Parameters override for this instance
    pub parameters: HashMap<String,ExprTokens>,
    /// Suffix to add to the name of the generated files
    pub suffixes: HashMap<String,SuffixInfo>,

}

impl RifmuxItem {
    pub fn new(info:RifmuxItemTuple, group: &str) -> RifmuxItem {
        let addr_info = info.2.unwrap_or_default();
        RifmuxItem {
            name: info.0.to_owned(),
            group: group.to_owned(),
            rif_type: info.1,
            addr_kind: addr_info.0,
            addr: addr_info.1,
            description: info.3.unwrap_or("").into(),
            parameters: HashMap::new(),
            suffixes: HashMap::new(),
        }
    }

    pub fn add_param(&mut self, key: &str, expr: ExprTokens) {
        self.parameters.insert(key.to_owned(), expr);
    }

    pub fn add_suffix(&mut self, key_val: (Option<&str>, SuffixInfo)) {
        self.suffixes.insert(
            key_val.0.unwrap_or_default().to_owned(),
            key_val.1
        );
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AddressOffset {
    Value(u64),
    Param(String)
}

impl Default for AddressOffset {
    fn default() -> Self {
        AddressOffset::Value(0)
    }
}

impl AddressOffset {
    pub fn value(&self, params: &ParamValues) -> u64 {
        match self {
            AddressOffset::Value(v) => *v,
            AddressOffset::Param(n) => *params.get(n).unwrap() as u64,
        }

    }
}

#[derive(Clone, Debug)]
/// Group instances with a common offset under a common name prefix
pub struct RifmuxGroup {
    /// Name of the RIF instance
    pub name: String,
    /// Addressing scheme used: absolute or relative
    pub addr_kind: AddressKind,
    /// Address of the instance
    pub addr: AddressOffset,
    /// Description
    pub description: Description,
}

pub type RifmuxGroupTuple<'a> = (&'a str, AddressKind, AddressOffset, Option<&'a str>);
impl<'a> From<RifmuxGroupTuple<'a>> for RifmuxGroup {
    fn from(info: RifmuxGroupTuple) -> RifmuxGroup {
        RifmuxGroup {
            name: info.0.to_owned(),
            addr_kind: info.1,
            addr: info.2,
            description: info.3.unwrap_or("").into(),
        }
    }
}


#[derive(Clone, Debug)]
pub struct RifmuxTop {
    /// Name of the RIF instance
    pub name: String,
    /// Prefixes used for the instances signals
    pub prefixes: BTreeMap<String,String>,
}

impl RifmuxTop {
    pub fn new<S>(name: S) -> Self where S: Into<String> {
        RifmuxTop {
            name: name.into(),
            prefixes: BTreeMap::new()
        }
    }
}
