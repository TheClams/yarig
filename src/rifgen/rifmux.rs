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
    /// Default access enable input for all instances (can be overridden by a group or an instance)
    pub enable: bool,
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
            enable: false,
        }
    }

    /// Check if the access to an instance is controlled by an enable input
    /// The instance setting has priority over the group, which has priority over the rifmux
    pub fn has_access_enable(&self, item: &RifmuxItem) -> bool {
        item.enable
            .or_else(|| self.groups.iter().find(|g| g.name==item.group).and_then(|g| g.enable))
            .unwrap_or(self.enable)
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

/// Artifact (file and/or name) on which a suffix can be applied
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuffixTarget {
    /// RTL module name, and the file containing it
    RtlType,
    /// RTL package name, and the file containing it
    RtlPkg,
    /// Name of the files generated by the documentation targets
    DocFile,
    /// Type name used inside the documentation
    DocType,
    /// Name of the files generated by the software targets (C, Python, SVD, IP-XACT)
    Sw,
    /// Name of the files generated by the simulation targets (RAL)
    Sim,
}

/// Selection of suffix targets, as written in the configuration or on the command-line.
/// Some of the selections are shortcuts selecting several targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuffixTargetSel {
    /// Both RTL module and package
    Rtl,
    /// RTL module only (name and file)
    RtlType,
    /// RTL package only (name and file)
    RtlPkg,
    /// Both type name and file name of the documentation
    Doc,
    /// File name of the documentation
    DocFile,
    /// Type name inside the documentation
    DocType,
    /// File name of software targets
    Sw,
    /// File name of simulation targets
    Sim,
}

impl FromStr for SuffixTargetSel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rtl"      => Ok(SuffixTargetSel::Rtl),
            "rtl_type" => Ok(SuffixTargetSel::RtlType),
            "rtl_pkg"  => Ok(SuffixTargetSel::RtlPkg),
            "doc"      => Ok(SuffixTargetSel::Doc),
            "doc_file" => Ok(SuffixTargetSel::DocFile),
            "doc_type" => Ok(SuffixTargetSel::DocType),
            "sw"       => Ok(SuffixTargetSel::Sw),
            "sim"      => Ok(SuffixTargetSel::Sim),
            _ => Err(format!("Unknown suffix target '{s}': expecting rtl, rtl_type, rtl_pkg, doc, doc_file, doc_type, sw or sim"))
        }
    }
}

impl<'de> serde::Deserialize<'de> for SuffixTargetSel {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <String as serde::Deserialize>::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Set of artifacts on which suffixes are applied by default.
///
/// The default value is the historical behavior: the suffix is added to the RTL module and to the name of all generated files,
/// but not to the RTL package nor to the type name inside the documentation.
/// Both can still be enabled for a specific suffix with its `pkg`/`doc`/`doc_type` flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SuffixTargets {
    rtl_type: bool,
    rtl_pkg: bool,
    doc_file: bool,
    doc_type: bool,
    sw: bool,
    sim: bool,
}

impl Default for SuffixTargets {
    fn default() -> Self {
        SuffixTargets {rtl_type: true, rtl_pkg: false, doc_file: true, doc_type: false, sw: true, sim: true}
    }
}

impl SuffixTargets {

    /// No target selected
    fn none() -> Self {
        SuffixTargets {rtl_type: false, rtl_pkg: false, doc_file: false, doc_type: false, sw: false, sim: false}
    }

    /// Build from a list of selection
    pub fn from_sel(sel: &[SuffixTargetSel]) -> Self {
        let mut t = Self::none();
        for s in sel {
            match s {
                SuffixTargetSel::Rtl     => {t.rtl_type = true; t.rtl_pkg = true;}
                SuffixTargetSel::RtlType => t.rtl_type = true,
                SuffixTargetSel::RtlPkg  => t.rtl_pkg = true,
                SuffixTargetSel::Doc     => {t.doc_file = true; t.doc_type = true;}
                SuffixTargetSel::DocFile => t.doc_file = true,
                SuffixTargetSel::DocType => t.doc_type = true,
                SuffixTargetSel::Sw      => t.sw = true,
                SuffixTargetSel::Sim     => t.sim = true,
            }
        }
        t
    }

    /// Suffix applied only to the RTL module (legacy behavior of option `suffix_rtl_only`)
    pub fn rtl_only() -> Self {
        SuffixTargets {rtl_type: true, ..Self::none()}
    }

}

/// Suffix Information: name, position and artifacts on which it applies
#[derive(Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct SuffixInfo {
    /// Suffix name
    pub name: String,
    /// When true place suffix just before _rif (if name ends in _rif)
    pub alt_pos: bool,
    /// Enable suffix on RTL package name
    pub pkg: bool,
    /// Enable suffix on both file name and type name of the documentation
    pub doc: bool,
    /// Enable suffix on the type name of the documentation (file name is not changed)
    pub doc_type: bool,
    /// Default targets, set from the global configuration
    #[serde(skip)]
    pub targets: SuffixTargets,
}

impl SuffixInfo {
    pub fn new(name: String, alt_pos: bool, pkg: bool) -> Self {
        SuffixInfo {name, alt_pos, pkg, ..Default::default()}
    }

    /// Enable suffix on documentation file and type name (`doc`) or on its type name only (`doc_type`)
    pub fn with_doc(mut self, doc: bool, doc_type: bool) -> Self {
        self.doc = doc;
        self.doc_type = doc_type;
        self
    }

    /// Set the targets enabled by default, on top of the flags of this suffix
    pub fn with_targets(mut self, targets: SuffixTargets) -> Self {
        self.targets = targets;
        self
    }

    /// Return true when the suffix applies to a given target
    pub fn applies_to(&self, target: SuffixTarget) -> bool {
        match target {
            SuffixTarget::RtlType => self.targets.rtl_type,
            SuffixTarget::RtlPkg  => self.targets.rtl_pkg || self.pkg,
            SuffixTarget::DocFile => self.targets.doc_file || self.doc,
            SuffixTarget::DocType => self.targets.doc_type || self.doc || self.doc_type,
            SuffixTarget::Sw      => self.targets.sw,
            SuffixTarget::Sim     => self.targets.sim,
        }
    }

    /// Add the suffix to a name when it applies to the target
    pub fn apply(&self, name: &str, target: SuffixTarget) -> String {
        if !self.applies_to(target) {
            name.to_owned()
        } else if self.alt_pos && name.ends_with("_rif") {
            format!("{}_{}_rif", &name[..(name.len()-4)], self.name)
        } else {
            format!("{name}_{}", self.name)
        }
    }
}

impl FromStr for SuffixInfo {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        suffix_info(s).map_err(|_| format!("Unable to parse {s} as a SuffixInfo. Format is suffix_name(alt,pkg,doc,doc_type) with all flags optional."))
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
    /// Access to the instance is controlled by an enable input (None to inherit from the group/rifmux)
    pub enable: Option<bool>,
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
            enable: None,
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
    /// Access to all instances of the group is controlled by an enable input (None to inherit from the rifmux)
    pub enable: Option<bool>,
}

pub type RifmuxGroupTuple<'a> = (&'a str, Address, Option<&'a str>);
impl From<RifmuxGroupTuple<'_>> for RifmuxGroup {
    fn from(info: RifmuxGroupTuple) -> RifmuxGroup {
        RifmuxGroup {
            name: info.0.to_owned(),
            addr: info.1,
            description: info.2.unwrap_or("").into(),
            enable: None,
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
