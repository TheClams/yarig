use std::collections::BTreeMap;
use std::str::FromStr;

use crate::parser::parser_expr::ExprTokens;

use super::{order_dict::OrderDict, Description, EnumDef, RifPage};

#[derive(Clone, Debug, PartialEq)]
pub struct ResetDef {
    pub name: String,
    pub sync: bool,
    pub active_high: bool,
}

impl ResetDef {
    /// Create an asynchronous reset, active low with configurable name
    pub fn new(name: String) -> Self {
        ResetDef {name, sync: false, active_high: false}
    }

    /// Return a simple description
    pub fn desc(&self) -> String {
        format!("{}synchronous reset, active {}",
            if self.sync { "" } else { "a" },
            if self.active_high { "high" } else { "low" },
        )
    }
}

impl Default for ResetDef {
    fn default() -> Self {
        Self::new("rst_n".to_owned())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ClockingInfo {
    pub clk : String,
    pub rst : ResetDef,
    pub en : String,
    pub clear : String,
}

impl Default for ClockingInfo {
    fn default() -> Self {ClockingInfo{clk:"clk".to_owned(), rst: Default::default(), en: "".to_owned(), clear: "".to_owned()}}
}


#[derive(Clone, Debug, PartialEq, Default)]
/// Supported processor interface kind to control register acces
pub enum Interface { #[default]
    /// Basic interface with memory like scheme
    Default,
    /// AMBA advanced Peripheral Bus
    Apb,
    /// Auxiliary peripheral bus
    Uaux,
    /// Custom interface
    Custom(String)
}


impl FromStr for Interface {
    type Err = std::io::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "default" => Ok(Interface::Default),
            "apb"     => Ok(Interface::Apb),
            "uaux"    => Ok(Interface::Uaux),
            custom    => Ok(Interface::Custom(custom.to_owned())),
        }
    }
}

impl Interface {
    pub fn name(&self) -> &str {
        match self {
            Interface::Default => "rif",
            Interface::Apb => "apb",
            Interface::Uaux => "uaux",
            Interface::Custom(n) => n,
        }
    }

    pub fn is_default(&self) -> bool {
        *self==Interface::Default
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenericRange {
    pub min : u8,
    pub max : u8,
    pub default : u8,
}

impl From<Vec<u8>> for GenericRange {
    fn from(v: Vec<u8>) -> GenericRange {
        match v.len() {
            // Empty vec ? just set everything to 1, should never happen
            0 => GenericRange{min:1, default:1, max:1},
            // Only one value => default==max
            1 => GenericRange{min:1, default:v[0], max:v[0]},
            // Two values => default and max
            2 => GenericRange{min:1, default:v[0], max:v[1]},
            // 3 or more ? min, default and max
            _ => GenericRange{min:v[0], default:v[1], max:v[2]},
        }
    }
}


#[derive(Clone, Debug)]
pub struct Rif {
    /// Type name
    pub name: String,
    /// Address bus width
    pub addr_width: u8,
    /// Data bus width
    pub data_width: u8,
    /// Top description
    pub description: Description,
    /// Software interface
    pub interface: Interface,
    /// Suffix also apply on package
    pub suffix_pkg: bool,
    /// Software interface clock definition
    pub sw_clocking: ClockingInfo,
    /// Hardware interface clock definition
    pub hw_clocking: Vec<ClockingInfo>,
    /// Register pages
    pub pages: Vec<RifPage>,
    /// Enum definition
    pub enum_defs: Vec<EnumDef>,
    /// Parameters definition
    pub parameters: OrderDict<String,ExprTokens>,
    /// Generics definition
    pub generics: BTreeMap<String,GenericRange>,
    /// Extra Custom information
    pub info: BTreeMap<String,String>,
}
impl Rif {
    pub fn new<S>(name: S) -> Self where S: Into<String> {
        Rif {
            name:name.into(),
            addr_width: 16,
            data_width: 32,
            description: "".into(),
            suffix_pkg: false,
            interface: Interface::Default,
            sw_clocking: ClockingInfo::default(),
            hw_clocking: Vec::new(),
            pages: Vec::new(),
            enum_defs: Vec::new(),
            parameters: OrderDict::new(),
            generics: BTreeMap::new(),
            info: BTreeMap::new(),
        }
    }

    pub fn add_param(&mut self, key: &str, expr: ExprTokens) {
        self.parameters.insert(key.to_owned(), expr);
    }

    pub fn add_generic(&mut self, key_val:(&str, GenericRange)) {
        self.generics.insert(key_val.0.to_owned(), key_val.1);
    }

    pub fn add_info(&mut self, key_val:(&str, &str)) {
        self.info.insert(key_val.0.to_owned(), key_val.1.to_owned());
    }

    pub fn set_hw_clk(&mut self, names:Vec<&str>) {
        if self.hw_clocking.is_empty() {
            self.hw_clocking = names.into_iter().map(|n| ClockingInfo{ clk: n.to_owned(), ..Default::default() }).collect();
        } else {
            self.hw_clocking.iter_mut().zip(names).for_each(|(hw, n)| hw.clk = n.to_owned());
        }
    }

    pub fn set_hw_clken(&mut self, names:Vec<&str>) {
        if self.hw_clocking.is_empty() {
            self.hw_clocking = names.into_iter().map(|n| ClockingInfo{ en: n.to_owned(), ..Default::default() }).collect();
        } else {
            self.hw_clocking.iter_mut().zip(names).for_each(|(hw, en)| hw.en = en.to_owned());
        }
    }

    pub fn set_hw_clear(&mut self, names:Vec<&str>) {
        if self.hw_clocking.is_empty() {
            self.hw_clocking = names.into_iter().map(|clear| ClockingInfo{ clear: clear.to_owned(), ..Default::default() }).collect();
        } else {
            self.hw_clocking.iter_mut().zip(names).for_each(|(hw, clear)| hw.clear = clear.to_owned());
        }
    }

    pub fn set_hw_rst(&mut self, rst: ResetDef) {
        if self.hw_clocking.is_empty() {
            self.hw_clocking = vec![ClockingInfo{rst, ..Default::default() }];
        } else {
            self.hw_clocking.iter_mut().for_each(|hw| hw.rst = rst.clone());
        }
    }
}
