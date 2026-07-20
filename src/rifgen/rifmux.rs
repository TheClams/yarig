use std::{collections::{BTreeMap, HashMap}, str::FromStr};

use serde_derive::Deserialize;

use crate::parser::{parser_expr::{ExprTokens, ParamValues}, suffix_info};
use crate::rifgen::ResetDef;

use super::{Address, ClockingInfo, DataWidth, Description, GenericRange, GenericValues, Interface, order_dict::OrderDict};

#[derive(Clone, Debug)]
pub struct Rifmux {
    /// Type name
    pub name: String,
    /// Address bus width
    pub addr_width: u8,
    /// Data bus width
    pub data_width: DataWidth,
    /// Software clocking defintion (clock, reset, enable)
    pub sw_clocking: Vec<ClockingInfo>,
    /// Hardware Interface
    pub interface: Interface,
    /// Items inside Rifmux (Rif or other rifmux)
    pub items: Vec<RifmuxItem>,
    /// Parameter definition
    pub parameters: OrderDict<String,ExprTokens>,
    /// Generics definition
    pub generics: GenericValues,
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
    /// Create an empty RifMux definition
    pub fn new<S>(name: S) -> Self where S: Into<String> {
        Rifmux {
            name: name.into(),
            addr_width: 16,
            data_width: DataWidth::default(),
            interface: Interface::Default,
            sw_clocking: Vec::new(),
            items: vec![],
            groups: vec![],
            description: "".into(),
            parameters: OrderDict::new(),
            generics: OrderDict::new(),
            info: HashMap::new(),
            top: None,
        }
    }

    /// Update the info dictionnary
    pub fn add_info(&mut self, key_val:(&str, &str)) {
        self.info.insert(key_val.0.to_owned(), key_val.1.to_owned());
    }

    /// Add a parameter definition
    pub fn add_param(&mut self, key: &str, expr: ExprTokens) {
        self.parameters.insert(key.to_owned(), expr);
    }

    /// Add a generic definition
    pub fn add_generic(&mut self, key_val:(&str, GenericRange)) {
        self.generics.insert(key_val.0.to_owned(), key_val.1);
    }

    /// Add a prefix definition for the top instance
    pub fn add_top_prefix(&mut self, key: &str, val: &str) {
        if let Some(ref mut top) = self.top {
            top.prefixes.insert(key.to_owned(),val.to_owned());
        }
    }

    /// Set the available software clocks
    pub fn set_sw_clk(&mut self, names:Vec<&str>) {
        names.into_iter().enumerate().for_each(|(i, n)| {
            if let Some(sw) = self.sw_clocking.get_mut(i) {
                sw.clk = n.to_owned();
            } else {
                self.sw_clocking.push(ClockingInfo { clk: n.to_owned(), ..Default::default() });
            }
        });
    }

    /// Set the clock enable corresponding to each defined software clock
    pub fn set_sw_clken(&mut self, names:Vec<&str>) {
        names.into_iter().enumerate().for_each(|(i, n)| {
            if let Some(sw) = self.sw_clocking.get_mut(i) {
                sw.en = n.to_owned();
            } else {
                let last = self.sw_clocking.last().cloned().unwrap_or_default();
                self.sw_clocking.push(ClockingInfo { en: n.to_owned(), ..last });
            }
        });
    }

    /// Set the reset corresponding to each defined software clock
    pub fn set_sw_rst(&mut self, idx: usize, rst: ResetDef) {
        if idx == 0 {
            if self.sw_clocking.is_empty() {
                self.sw_clocking = vec![ClockingInfo { rst, ..Default::default() }];
            } else {
                self.sw_clocking.iter_mut().for_each(|sw| sw.rst = rst.clone());
            }
        } else if let Some(sw) = self.sw_clocking.get_mut(idx) {
            sw.rst = rst;
        } else {
            let last = self.sw_clocking.last().cloned().unwrap_or_default();
            self.sw_clocking.push(ClockingInfo { rst, ..last });
        }
    }

}

#[derive(Clone, Debug, PartialEq)]
pub enum RifType {Rif(String), Ext(u8)}
/// Tuple from parser
/// Values are: instance name, array size, type name, group name, addressing scheme and address
pub type RifmuxItemTuple<'a> = (&'a str, RifType, Option<Address>, Option<&'a str>);

/// Suffix Information: name, position and if used for RTL package
#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct SuffixInfo {
    /// Suffix name
    pub name: String,
    /// When true place suffix just before _rif (if name ends in _rif)
    pub alt_pos: bool,
    /// Enable suffix on package name
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
pub enum ItemOptional {None, Enable, Expr(ExprTokens)}

impl ItemOptional {
    /// Get Expression token when type is Expr
    pub fn expr(&self) -> Option<&ExprTokens> {
        match self {
            ItemOptional::Expr(e) => Some(e),
            _ => None
        }
    }

    /// Check if optional is Enable
    pub fn is_enable(&self) -> bool {
        matches!(self, &ItemOptional::Enable)
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
    /// Address of the instance
    pub addr: Address,
    /// Description
    pub description: Description,
    /// Parameters override for this instance
    pub parameters: OrderDict<String, ExprTokens>,
    /// Suffix to add to the name of the generated files
    pub suffixes: HashMap<String,SuffixInfo>,
    /// Indicates the page instance is controlled by a parameter or generic
    pub optional: ItemOptional,

}

impl RifmuxItem {
    /// Create a rifmux item from parsed tupple
    pub fn new(info:RifmuxItemTuple, group: &str) -> RifmuxItem {
        let addr_info = info.2.unwrap_or_default();
        RifmuxItem {
            name: info.0.to_owned(),
            group: group.to_owned(),
            rif_type: info.1,
            addr: addr_info,
            optional: ItemOptional::None,
            description: info.3.unwrap_or("").into(),
            parameters: OrderDict::new(),
            suffixes: HashMap::new(),
        }
    }

    /// Add parameter over-ride
    pub fn add_param(&mut self, key: &str, expr: ExprTokens) {
        self.parameters.insert(key.to_owned(), expr);
    }

    /// Add Suffix definition
    pub fn add_suffix(&mut self, key_val: (Option<&str>, SuffixInfo)) {
        self.suffixes.insert(
            key_val.0.unwrap_or_default().to_owned(),
            key_val.1
        );
    }

    /// High when register instance is deactivated
    pub fn is_disabled(&self, params: &ParamValues) -> bool {
        if self.optional.is_enable() {false}
        else if let Some(e) = self.optional.expr() {e.eval(params) == Ok(0)}
        else {false}
    }

}

#[derive(Clone, Debug)]
/// Group instances with a common offset under a common name prefix
pub struct RifmuxGroup {
    /// Name of the RIF instance
    pub name: String,
    /// Address of the instance
    pub addr: Address,
    /// Description
    pub description: Description,
}

pub type RifmuxGroupTuple<'a> = (&'a str, Address, Option<&'a str>);
impl From<RifmuxGroupTuple<'_>> for RifmuxGroup {
    fn from(info: RifmuxGroupTuple) -> RifmuxGroup {
        RifmuxGroup {
            name: info.0.to_owned(),
            addr: info.1,
            description: info.2.unwrap_or("").into(),
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
