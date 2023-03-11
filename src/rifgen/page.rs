use std::collections::HashMap;

use crate::parser::{get_rif, parser_expr::ExprTokens};

use super::{Access, ClkEn, Description, InterruptRegKind, Limit, RegDef, RegDefOrIncl, ResetVal, Rif, Visibility};

#[derive(Clone, Debug, PartialEq)]
pub struct RifPage{
    /// Page name
    pub name: String,
    /// Address offset for the page
    pub addr: u64,
    /// Default clock enable for the page
    pub clk_en : ClkEn,
    /// Page description
    pub description: Description,
    /// Indicates the page instance is controlled by a parameter
    pub optional: String,
    /// List of register definition for the page
    pub registers: Vec<RegDefOrIncl>,
    /// List of register instance for the page
    pub instances: Vec<RegInst>,
    /// Indicates all register are instantiated in order
    pub inst_auto: bool,
    /// Indicates the page logic is handled externally
    pub external: bool,
    /// Indicate the address width associated with the page (mandatory for external, optional otherwise)
    pub addr_width: u8,
}
impl RifPage {
    pub fn new<S>(name: S) -> Self where S: Into<String> {
        RifPage {
            name:name.into(),
            addr: 0,
            addr_width: 0,
            clk_en: ClkEn::Default,
            description: "".into(),
            optional: "".to_owned(),
            registers: vec![],
            instances: vec![],
            inst_auto: false,
            external: false,
        }
    }

    pub fn find_regdef<'a>(&'a self, name: &'a str, rifs: &'a HashMap<String, Rif>) -> Option<(&'a RegDef,InterruptRegKind,usize)> {
        for r in self.registers.iter() {
            match r {
                RegDefOrIncl::Include(inc) => {
                    let s: Vec<&str> = inc.split('.').collect();
                    if s.len()<3 || s.get(2)==Some(&name) || s.get(2)==Some(&"*") {
                        if let Some(rif_def) = get_rif(rifs,s[0]) {
                            for inc_page in rif_def.pages.iter() {
                                if s.len()==1 || s.get(1)==Some(&inc_page.name.as_str()) {
                                    let d = inc_page.find_regdef(name, rifs);
                                    if d.is_some() {
                                        return d;
                                    }
                                }
                            }
                        }
                    }
                }
                RegDefOrIncl::Def(d) => {
                    if d.name == name {
                        let kind = if d.interrupt.is_empty() {InterruptRegKind::None} else {InterruptRegKind::Base};
                        return Some((&d,kind,0));
                    }
                    // Check interrupt register
                    else if !d.interrupt.is_empty() && name.starts_with(&d.name) {
                        let name_suffix = &name[d.name.len()..];
                        for (idx,info) in d.interrupt.iter().enumerate() {
                            let intr_name = if info.name.is_empty() {"".to_owned()} else {format!("_{}",info.name)};
                            // Check if enable interrupt is enabled
                            if info.enable.is_some() && name_suffix == format!("{}_en",intr_name) {
                                return Some((&d,InterruptRegKind::Enable,idx));
                            }
                            // Check if mask interrupt is enabled
                            if info.mask.is_some() && name_suffix == format!("{}_mask",intr_name) {
                                return Some((&d,InterruptRegKind::Mask,idx));
                            }
                            // Check if mask interrupt is enabled
                            if info.pending && name_suffix == format!("{}_pending",intr_name) {
                                return Some((&d,InterruptRegKind::Pending,idx));
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Find register instance override information
    pub fn find_reg_inst<'a>(&'a self, name: &'a str) -> Option<&'a RegInst> {
        for inst in self.instances.iter() {
            if inst.type_name != name {
                continue;
            }
            return Some(inst);
        }
        None
    }
}


#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Default)]
/// Addressing scheme for register instances
pub enum AddressKind {#[default]
    /// Absolute value
    Absolute,
    /// Relative to the last absolute value provided
    Relative,
    /// Like Relative but also set current address as absolute value
    RelativeSet
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum ResetValOverride {
    /// No override
    None,
    /// Relative to the last absolute value provided
    Reset(ResetVal),
    /// Like Relative but also set current address as absolute value
    Disable(ResetVal),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldOverride {
    /// Register description override
    pub description: Option<Description>,
    /// Indicates the register instance is controlled by a parameter
    pub optional: ExprTokens,
    /// Change the register visibility
    pub visibility: Option<Visibility>,
    /// Reset override
    pub reset:ResetValOverride,
    /// Limit override
    pub limit: Option<Limit>,
    /// Info override
    pub info: HashMap<String,String>,
}

impl Default for FieldOverride {
    fn default() -> Self {
        FieldOverride {
            description: None,
            optional   : ExprTokens::new(0),
            visibility : None,
            reset      : ResetValOverride::None,
            limit: None,
            info: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RegOverride {
    /// Register description override
    pub description: Option<Description>,
    /// Indicates the register instance is controlled by a parameter
    pub optional: ExprTokens,
    /// Change the register visibility
    pub visibility: Option<Visibility>,
    /// Override the hardware access to the register
    pub hw_acc: Option<Access>,
    /// Field settings override
    pub fields : HashMap<String,FieldOverride>,
}

/// Index of the register to override. None if register is not an array or to override all registers
pub type OptArrayIndex = Option<u16>;
/// Field name of the register to override. None if override is for the register it-self
pub type FieldOverrideIndex = Option<String>;
pub type OverrideIndex = (OptArrayIndex, FieldOverrideIndex, OptArrayIndex);
pub type RegOverrideDict = HashMap<OptArrayIndex,RegOverride>;

/// Tuple from parser
/// Values are: instance name, array size, type name, group name, addressing scheme and address
pub type RegInstTuple<'a> = (&'a str, ExprTokens, Option<&'a str>, Option<&'a str>, Option<(AddressKind, u64)>);

#[derive(Clone, Debug, PartialEq)]
/// Register instance description
pub struct RegInst {
    /// Name of the register instance
    pub inst_name: String,
    /// Name of the register type
    pub type_name: String,
    /// Name of the hardware structure the register is part of (only needed when the structure spans multiple registers)
    pub group_name: String,
    /// Addressing scheme used: absolute or relative
    pub addr_kind: AddressKind,
    /// Address of the instance
    pub addr: u64,
    /// Number of the instance: can be an integer, a parameter or a generic
    pub array: ExprTokens,
    /// Register settings override
    pub reg_override : RegOverrideDict,
}

impl<'a> From<RegInstTuple<'a>> for RegInst {
    fn from(info:RegInstTuple) -> RegInst {
        let addr_info = info.4.unwrap_or((AddressKind::RelativeSet,0));
        let default_group = if info.2.is_some() {info.0} else {""};
        RegInst {
            inst_name: info.0.to_owned(),
            type_name: info.2.unwrap_or(info.0).to_owned(),
            group_name:info.3.unwrap_or(default_group).to_owned(),
            addr_kind: addr_info.0,
            addr: addr_info.1,
            array: info.1,
            reg_override: HashMap::new()
        }
    }
}

impl RegInst {

    fn get_field_ovr<'a>(reg: &'a mut RegOverride, name: &str, idx: OptArrayIndex) -> &'a mut FieldOverride{
        let mut field_name = name.to_owned();
        if let Some(field_idx) = idx {
            field_name.push_str(&format!("[{field_idx}]"));
        }
        reg.fields.entry(field_name).or_default()
    }

    pub fn desc_updt(&mut self, idx: &OverrideIndex, desc: &str) {
        let reg = self.reg_override.entry(idx.0).or_default();
        match &idx.1 {
            Some(name) => {
                let field = Self::get_field_ovr(reg, name, idx.2);
                if field.description.is_none() {
                    field.description = Some(desc.into());
                } else {
                    field.description.as_mut().unwrap().updt(desc);
                }
            }
            None => {
                if reg.description.is_none() {
                    reg.description = Some(desc.into());
                } else {
                    reg.description.as_mut().unwrap().updt(desc);
                }
            }
        }
    }

    pub fn set_optional(&mut self, idx: &OverrideIndex, v: ExprTokens) {
        let reg = self.reg_override.entry(idx.0).or_default();
        match &idx.1 {
            Some(name) => {
                let field = Self::get_field_ovr(reg, name, idx.2);
                field.optional = v;
            },
            None => reg.optional = v,
        }
    }

    pub fn set_visibility(&mut self, idx: &OverrideIndex, v: Visibility) {
        let reg = self.reg_override.entry(idx.0).or_default();
        match &idx.1 {
            Some(name) => {
                let field = Self::get_field_ovr(reg, name, idx.2);
                field.visibility = Some(v);
            },
            None => reg.visibility = Some(v),
        }
    }

    pub fn set_hw_acc(&mut self, idx: &OverrideIndex, v: Access) {
        let reg = self.reg_override.entry(idx.0).or_default();
        reg.hw_acc = Some(v);
    }

    pub fn set_reset(&mut self, idx: &OverrideIndex, v: ResetVal) {
        let reg = self.reg_override.entry(idx.0).or_default();
        match &idx.1 {
            Some(name) => {
                let field = Self::get_field_ovr(reg, name, idx.2);
                field.reset = ResetValOverride::Reset(v);
            },
            None => {},
        }
    }

    pub fn set_limit(&mut self, idx: &OverrideIndex, limit: Limit) {
        let reg = self.reg_override.entry(idx.0).or_default();
        match &idx.1 {
            Some(name) => {
                let field = Self::get_field_ovr(reg, name, idx.2);
                field.limit = Some(limit);
            },
            None => {},
        }
    }

    pub fn add_info(&mut self, idx: &OverrideIndex, key_val:(&str, &str)) {
        let reg = self.reg_override.entry(idx.0).or_default();
        match &idx.1 {
            Some(name) => {
                let field = Self::get_field_ovr(reg, name, idx.2);
                field.info.insert(key_val.0.to_owned(), key_val.1.to_owned());
            },
            None => {panic!("Unsuported info on reg");},
        }
    }

}
