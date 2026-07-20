use crate::error::RifError;
use crate::parser::parser_expr::ExprTokens;

use super::{DataWidth, EnumEntry};
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
    fn default() -> Self {
        ClockingInfo{
            clk:"clk".to_owned(),
            rst: Default::default(),
            en: "".to_owned(),
            clear: "".to_owned()
        }
    }
}

impl ClockingInfo {
    pub fn new_apb() -> Self{
        ClockingInfo{
            clk:"pclk".to_owned(),
            rst: ResetDef::new("presetn".to_owned()),
            en: "".to_owned(),
            clear: "".to_owned()
        }
    }
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
    Custom(String,String)
}


impl From<&str> for Interface {

    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "apb"  => Interface::Apb,
            "uaux" => Interface::Uaux,
            _      => Interface::Default,
        }
    }
}

impl Interface {
    /// Create a custom interface with a name and path to an RTL implementation
    pub fn new_custom(name: &str, filename: &str) -> Self {
        Self::Custom(name.to_owned(), filename.to_owned())
    }

    /// Update the path of a custom interface
    pub fn set_path(&mut self, path: &str) {
        if let Interface::Custom(_, path_l) = self {
            path_l.clear();
            path_l.push_str(path);
        }
    }

    /// Retrieve path of custom interface
    pub fn get_path(&self) -> Option<&str> {
        if let Interface::Custom(_, path) = &self {
            Some(path.as_str())
        } else {
            None
        }
    }

    /// Return interface as a name
    pub fn name(&self) -> &str {
        match &self {
            Interface::Default => "rif",
            Interface::Apb     => "apb",
            Interface::Uaux    => "uaux",
            Interface::Custom(n, _) => n,
        }
    }

    /// Check if inertface variant is Default
    pub fn is_default(&self) -> bool {
        *self==Interface::Default
    }
}

impl<'de> serde::Deserialize<'de> for Interface {
    fn deserialize<D: serde::Deserializer<'de> >(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Ok(Interface::from(s.as_ref()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GenericRange {
    pub min : u16,
    pub max : u16,
    pub default : u16,
    pub desc: Option<String>,
}

impl GenericRange {
    pub fn binary(init: u8, desc: Option<String>) -> Self {
        GenericRange {min:0, default: init.into(), max:1, desc}
    }
}

impl From<(Vec<u16>, Option<&str>)> for GenericRange {
    fn from(v: (Vec<u16>, Option<&str>)) -> GenericRange {
        let desc =  v.1.map(|d| d.to_owned());
        match v.0.len() {
            // Empty vec ? just set everything to 1, should never happen
            0 => GenericRange::binary(0, desc),
            // Only one value => default==max
            1 => {
                if v.0[0] < 2 {
                    GenericRange::binary(v.0[0] as u8, desc)
                } else {
                    GenericRange{min:1, default:v.0[0], max:v.0[0], desc}
                }
            }
            // Two values => default and max
            2 => GenericRange{min:1, default:v.0[0], max:v.0[1], desc},
            // 3 or more ? min, default and max
            _ => GenericRange{min:v.0[0], default:v.0[1], max:v.0[2], desc},
        }
    }
}

pub type GenericValues = OrderDict<String,GenericRange>;

#[derive(Clone, Debug)]
pub struct Rif {
    /// Type name
    pub name: String,
    /// Address bus width
    pub addr_width: u8,
    /// Data bus width
    pub data_width: DataWidth,
    /// Top description
    pub description: Description,
    /// Software interface
    pub interface: Interface,
    /// Suffix also apply on package
    pub suffix_pkg: bool,
    /// Software interface clock definition
    pub sw_clocking: Vec<ClockingInfo>,
    /// Hardware interface clock definition
    pub hw_clocking: Vec<ClockingInfo>,
    /// Register pages
    pub pages: Vec<RifPage>,
    /// Enum definition
    pub enum_defs: Vec<EnumDef>,
    /// Parameters definition
    pub parameters: OrderDict<String,ExprTokens>,
    /// Generics definition
    pub generics: GenericValues,
    /// Extra Custom information
    pub info: OrderDict<String,String>,
}
impl Rif {
    /// Create an empty Rif definition
    pub fn new<S>(name: S) -> Self where S: Into<String> {
        Rif {
            name:name.into(),
            addr_width: 16,
            data_width: DataWidth::default(),
            description: "".into(),
            suffix_pkg: false,
            interface: Interface::Default,
            sw_clocking: Vec::new(),
            hw_clocking: Vec::new(),
            pages: Vec::new(),
            enum_defs: Vec::new(),
            parameters: OrderDict::new(),
            generics: OrderDict::new(),
            info: OrderDict::new(),
        }
    }

    /// Add a parameter definition
    pub fn add_param(&mut self, key: &str, expr: ExprTokens) {
        self.parameters.insert(key.to_owned(), expr);
    }

    /// Add a generic definition
    pub fn add_generic(&mut self, key_val:(&str, GenericRange)) {
        self.generics.insert(key_val.0.to_owned(), key_val.1);
    }

    /// Add an information entry
    pub fn add_info(&mut self, key_val:(&str, &str)) {
        self.info.insert(key_val.0.to_owned(), key_val.1.to_owned());
    }

    /// Add a new enum entry
    pub fn add_enum_entry(&mut self, name: &str, entry: EnumEntry) -> Result<(), RifError> {
        let Some(def) = self.enum_defs.iter_mut().find(|e| e.name==name) else {
            return Err(RifError::generic(&format!("Unable to find enum {name}")));
        };
        // Check if enum value already exist
        if let Some(e) = def.values.iter().find(|e| e.value==entry.value) {
            return Err(RifError::generic(&format!("Duplicated enum value for {} and {}", e.name, entry.name)));
        }
        def.values.push(entry);
        Ok(())
    }

    /// Set all the software clock available
    pub fn set_sw_clk(&mut self, names:Vec<&str>) {
        names.into_iter().enumerate().for_each(|(i, n)| {
            if let Some(sw) = self.sw_clocking.get_mut(i) {
                sw.clk = n.to_owned();
            } else {
                self.sw_clocking.push(ClockingInfo { clk: n.to_owned(), ..Default::default() });
            }
        });
    }

    /// Set the enable corresponding to each defined software clock
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

    /// Set the clear signal corresponding to each defined softtware clock
    pub fn set_sw_clear(&mut self, names:Vec<&str>) {
        names.into_iter().enumerate().for_each(|(i, n)| {
            if let Some(sw) = self.sw_clocking.get_mut(i) {
                sw.clear = n.to_owned();
            } else {
                let last = self.sw_clocking.last().cloned().unwrap_or_default();
                self.sw_clocking.push(ClockingInfo { clear: n.to_owned(), ..last });
            }
        });
    }

    /// Set a reset corresponding to a software clock
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

    /// Set all the hardware clock available
    pub fn set_hw_clk(&mut self, names:Vec<&str>) {
        names.into_iter().enumerate().for_each(|(i, n)| {
            if let Some(hw) = self.hw_clocking.get_mut(i) {
                hw.clk = n.to_owned();
            } else {
                self.hw_clocking.push(ClockingInfo { clk: n.to_owned(), ..Default::default() });
            }
        });
    }

    /// Set the enable corresponding to each defined hardware clock
    pub fn set_hw_clken(&mut self, names:Vec<&str>) {
        names.into_iter().enumerate().for_each(|(i, n)| {
            if let Some(hw) = self.hw_clocking.get_mut(i) {
                hw.en = n.to_owned();
            } else {
                let last = self.hw_clocking.last().cloned().unwrap_or_default();
                self.hw_clocking.push(ClockingInfo { en: n.to_owned(), ..last });
            }
        });
    }

    /// Set the clear signal corresponding to each defined hardtware clock
    pub fn set_hw_clear(&mut self, names:Vec<&str>) {
        names.into_iter().enumerate().for_each(|(i, n)| {
            if let Some(hw) = self.hw_clocking.get_mut(i) {
                hw.clear = n.to_owned();
            } else {
                let last = self.hw_clocking.last().cloned().unwrap_or_default();
                self.hw_clocking.push(ClockingInfo { clear: n.to_owned(), ..last });
            }
        });
    }

    /// Set a reset corresponding to a hardware clock
    pub fn set_hw_rst(&mut self, idx: usize, rst: ResetDef) {
        if idx == 0 {
            if self.hw_clocking.is_empty() {
                self.hw_clocking = vec![ClockingInfo { rst, ..Default::default() }];
            } else {
                self.hw_clocking.iter_mut().for_each(|hw| hw.rst = rst.clone());
            }
        } else if let Some(hw) = self.hw_clocking.get_mut(idx) {
            hw.rst = rst;
        } else {
            let last = self.hw_clocking.last().cloned().unwrap_or_default();
            self.hw_clocking.push(ClockingInfo { rst, ..last });
        }
    }
}
