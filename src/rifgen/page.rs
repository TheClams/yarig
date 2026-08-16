use std::collections::HashMap;

use crate::{comp::comp_inst::ArrayIdx, parser::{get_rif, parser_expr::{ExprTokens, ParamValues}}};

use super::{Access, ClkEn, DeclLine, Description, DescBlockKind, InterruptRegKind, LimitP, PropBlocks, PropLines, RegDef, RegDefOrIncl, ResetValP, Rif, Visibility};

/// Result of register definition search
/// Contains the register definition itself, information about the interrupt kind and its RIF source if included
pub struct RegDefMatch<'a> {
    pub def: &'a RegDef,
    pub incl: Option<String>,
    pub intr_kind: InterruptRegKind,
    pub intr_idx : usize,
}

impl<'a> RegDefMatch<'a> {
    fn new(def: &'a RegDef) -> Self {
        let intr_kind = if def.interrupt.is_empty() {InterruptRegKind::None} else {InterruptRegKind::Base};
        RegDefMatch {def, incl: None, intr_kind, intr_idx: 0}
    }

    fn new_intr(def: &'a RegDef, intr_kind: InterruptRegKind, intr_idx : usize) -> Self {
        RegDefMatch {def, incl: None, intr_kind, intr_idx}
    }

    fn with_inc(mut self, inc: &str) -> Self {
        self.incl = Some(inc.to_owned());
        self
    }

}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InstMode {
    /// Register are instantiated manually
    Manual,
    /// Registers are manually instantiated
    Automatic,
    /// Registers are manually instantiated using a different order on interrupts (mask before enable)
    AutoLegacy,
}

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
    pub inst_auto: InstMode,
    /// Indicates the page logic is handled externally
    pub external: bool,
    /// Indicate the address width associated with the page (mandatory for external, optional otherwise)
    pub addr_width: u8,
    /// Source line of the `instances:`.
    pub instances_decl: DeclLine,
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
            inst_auto: InstMode::Manual,
            external: false,
            instances_decl: DeclLine::default(),
        }
    }

    pub fn find_regdef<'a>(&'a self, name: &'a str, rifs: &'a HashMap<String, Rif>) -> Option<RegDefMatch<'a>> {
        for r in self.registers.iter() {
            match r {
                RegDefOrIncl::Include(inc) => {
                    let s: Vec<&str> = inc.split('.').collect();
                    if (s.len()<3 || s.get(2)==Some(&name) || s.get(2)==Some(&"*")) && let Some(rif_def) = get_rif(rifs,s[0]) {
                        for inc_page in rif_def.pages.iter() {
                            if (s.len()==1 || s.get(1)==Some(&inc_page.name.as_str())) && let Some(d) = inc_page.find_regdef(name, rifs) {
                                    return Some(d.with_inc(s[0]));
                            }
                        }
                    }
                }
                RegDefOrIncl::Def(d) => {
                    if d.name == name {
                        return Some(RegDefMatch::new(d));
                    }
                    // Check interrupt register
                    else if !d.interrupt.is_empty() && name.starts_with(&d.name) {
                        let name_suffix = &name[d.name.len()..];
                        for (idx,info) in d.interrupt.iter().enumerate() {
                            let intr_name = if info.name.is_empty() {"".to_owned()} else {format!("_{}",info.name)};
                            // Check if enable interrupt is enabled
                            if info.enable.is_some() && name_suffix == format!("{intr_name}_en") {
                                return Some(RegDefMatch::new_intr(d,InterruptRegKind::Enable,idx));
                            }
                            // Check if mask interrupt is enabled
                            if info.mask.is_some() && name_suffix == format!("{intr_name}_mask") {
                                return Some(RegDefMatch::new_intr(d,InterruptRegKind::Mask,idx));
                            }
                            // Check if mask interrupt is enabled
                            if info.pending && name_suffix == format!("{intr_name}_pending") {
                                return Some(RegDefMatch::new_intr(d,InterruptRegKind::Pending,idx));
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

    pub fn is_auto(&self) -> bool {
        matches!(self.inst_auto, InstMode::Automatic | InstMode::AutoLegacy)
    }

    pub fn is_auto_legacy(&self) -> bool {
        matches!(self.inst_auto, InstMode::AutoLegacy)
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

    /// Serialize back to `.rif` syntax (hexadecimal for a plain value, `$name` for a parameter).
    pub fn to_rif(&self) -> String {
        match self {
            AddressOffset::Value(v) => format!("0x{v:x}"),
            AddressOffset::Param(n) => format!("${n}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Address {
    pub kind: AddressKind,
    pub offset: AddressOffset,
}

impl Default for Address {
    fn default() -> Self {
        Address::new(AddressKind::RelativeSet, AddressOffset::Value(0))
    }
}

impl Address {
    /// Create Address with kind and offset
    pub fn new(kind: AddressKind, offset: AddressOffset) -> Self {
        Self {kind, offset}
    }

    /// Return the offset value (after interpreting parameters if any)
    pub fn value(&self, params: &ParamValues) -> u64 {
        self.offset.value(params)
    }

    /// Serialize back to `.rif` syntax: `@`/`@+`/`@+=` followed by the offset.
    pub fn to_rif(&self) -> String {
        let kind = match self.kind {
            AddressKind::Absolute => "@",
            AddressKind::Relative => "@+",
            AddressKind::RelativeSet => "@+=",
        };
        format!("{kind} {}", self.offset.to_rif())
    }
}

impl From<(AddressKind,AddressOffset)> for Address {
    fn from(value: (AddressKind,AddressOffset)) -> Self {
        Self::new(value.0, value.1)
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum ResetValOverride {
    /// No override
    None,
    /// Relative to the last absolute value provided
    Reset(ResetValP),
    /// Like Relative but also set current address as absolute value
    Disable(ResetValP),
}

/// Identifies a field-override sub-property
/// Array-indexed override (`[0,5].field.reset = ...`) not yet supported
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldOverrideProp {
    Description,
    Reset,
}

impl FieldOverrideProp {
    /// All managed sub-properties, in the canonical order they are emitted for a new override.
    pub const ALL: [FieldOverrideProp; 2] = [FieldOverrideProp::Description, FieldOverrideProp::Reset];
}

/// Source-tracking metadata attached to a parsed `FieldOverride`.
#[derive(Clone, Debug, Default)]
pub struct FieldOverrideSrc {
    ///Source line of each managed sub-property present at parse time.
    pub prop_lines: PropLines<FieldOverrideProp>,
    /// (start, end) source lines of this override's own description
    pub desc_blocks: PropBlocks<DescBlockKind>,
}

/// Source position never affects equality: see `field::SrcInfo`, which documents the same rule.
impl PartialEq for FieldOverrideSrc {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
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
    pub limit: Option<LimitP>,
    /// Info override
    pub info: HashMap<String,String>,
    /// Source-tracking metadata for round-tripping edits back to the `.rif` file.
    pub src: FieldOverrideSrc,
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
            src: FieldOverrideSrc::default(),
        }
    }
}

impl FieldOverride {
    /// Merge two override
    /// Every None/Empty override of self take value defined in the def override
    pub fn merge(&mut self, def: &Self) {
        if self.description.is_none() {
            self.description = def.description.clone();
        }
        if self.optional.is_empty() {
            self.optional = def.optional.clone();
        }
        if self.visibility.is_none() {
            self.visibility = def.visibility;
        }
        if self.limit.is_none() {
            self.limit = def.limit.clone();
        }
        if self.reset == ResetValOverride::None {
            self.reset = def.reset.clone();
        }
        for (k,v) in def.info.iter() {
            self.info.entry(k.clone()).or_insert(v.clone());
        }
    }

    /// Serialize property as its canonical `.rif` line for `field_name`, prefixed with `indent`.
    pub fn fmt_prop(&self, field_name: &str, prop: FieldOverrideProp, indent: &str, field_reset: &ResetValP) -> Option<String> {
        match prop {
            FieldOverrideProp::Description => {
                let d = self.description.as_ref()?;
                Some(format!("{indent}{field_name}.description {}", d.get_short(true)))
            }
            FieldOverrideProp::Reset => {
                let disabled = self.visibility == Some(Visibility::Disabled);
                match (&self.reset, disabled) {
                    (ResetValOverride::Reset(v), true) if v == field_reset => Some(format!("{indent}{field_name}.disable")),
                    (ResetValOverride::Reset(v), true) => Some(format!("{indent}{field_name}.disable = {}", v.to_rif())),
                    (ResetValOverride::None, true) => Some(format!("{indent}{field_name}.disable")),
                    (ResetValOverride::Reset(v), false) => Some(format!("{indent}{field_name}.reset = {}", v.to_rif())),
                    _ => None,
                }
            }
        }
    }

    /// Serialized all properties lines for `field_name` in canonical order.
    pub fn fmt_prop_all(&self, field_name: &str, indent: &str, field_reset: &ResetValP) -> Vec<String> {
        FieldOverrideProp::ALL.iter().filter_map(|&p| self.fmt_prop(field_name, p, indent, field_reset)).collect()
    }

    /// Serialize this override's description, prefixed with `indent`.
    /// `None` when there is nothing beyond the first line.
    pub fn fmt_desc_block(&self, indent: &str) -> Option<Vec<String>> {
        let rest = self.description.as_ref()?.get_split(true).1?;
        let body_indent = format!("{indent}  ");
        Some(rest.split('\n').map(|l| format!("{body_indent}{l}")).collect())
    }
}


/// Register-override properties
/// Array-indexed override not yet supported
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegOverrideProp {
    Description,
    Hw,
    Optional,
    OptionalAcc,
}

impl RegOverrideProp {
    /// All managed sub-properties, in the canonical order they are emitted for a new override.
    pub const ALL: [RegOverrideProp; 4] =
        [RegOverrideProp::Description, RegOverrideProp::Hw, RegOverrideProp::Optional, RegOverrideProp::OptionalAcc];
}

/// Source-tracking metadata attached to a parsed `RegOverride`.
#[derive(Clone, Debug, Default)]
pub struct RegOverrideSrc {
    /// Source line of each managed sub-property present at parse time.
    pub prop_lines: PropLines<RegOverrideProp>,
    /// (start, end) source lines of this override's own descriptions (public/private/interrupts...)
    pub desc_blocks: PropBlocks<DescBlockKind>,
}

/// Source position never affects equality: see `field::SrcInfo`, which documents the same rule.
impl PartialEq for RegOverrideSrc {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RegOverride {
    /// Register address override (For auto instance only)
    pub addr: Option<Address>,
    /// Register description override
    pub description: Option<Description>,
    /// Indicates the register instance is controlled by a parameter
    pub optional: ExprTokens,
    /// Access when register is optional and disabled
    pub optional_acc: Option<Access>,
    /// Change the register visibility
    pub visibility: Option<Visibility>,
    /// Override the hardware access to the register
    pub hw_acc: Option<Access>,
    /// Field settings override
    pub fields : HashMap<String,FieldOverride>,
    /// Source-tracking metadata for edition.
    pub src: RegOverrideSrc,
}

impl RegOverride {
    /// Merge two override
    /// Every None/Empty override of self take value defined in the def override
    pub fn merge(&self, def: Option<&Self>) -> Self {
        let mut merged = self.clone();
        if let Some(d) = def {
            if self.addr.is_none() {
                merged.addr = d.addr.clone();
            }
            if self.description.is_none() {
                merged.description = d.description.clone();
            }
            if self.optional.is_empty() {
                merged.optional = d.optional.clone();
            }
            if self.visibility.is_none() {
                merged.visibility = d.visibility;
            }
            if self.hw_acc.is_none() {
                merged.hw_acc = d.hw_acc;
            }
            // Field override: merge
            for (k,v) in merged.fields.iter_mut() {
                if let Some(def_v) = d.fields.get(k) {
                    v.merge(def_v);
                }
            }
            merged.optional_acc = d.optional_acc;
            d.fields.iter()
                .filter(|(k,_)| !self.fields.contains_key(*k))
                .for_each(|(k,v)| { merged.fields.insert(k.clone(), v.clone());});
        }
        merged
    }

    /// Serialize a property as its canonical `.rif` line, prefixed with `indent`.
    pub fn fmt_prop(&self, prop: RegOverrideProp, indent: &str) -> Option<String> {
        match prop {
            RegOverrideProp::Description => {
                let d = self.description.as_ref()?;
                Some(format!("{indent}description : {}", d.get_short(true)))
            }
            RegOverrideProp::Hw => self.hw_acc.map(|a| format!("{indent}hw {}", a.to_string().to_lowercase())),
            RegOverrideProp::Optional => (!self.optional.is_empty()).then(|| format!("{indent}optional : {}", self.optional.to_rif())),
            RegOverrideProp::OptionalAcc => self.optional_acc.map(|a| format!("{indent}optional_acc: {}", a.to_string().to_lowercase())),
        }
    }

    ///Serialized all override properties
    pub fn fmt_prop_all(&self, indent: &str) -> Vec<String> {
        RegOverrideProp::ALL.iter().filter_map(|&p| self.fmt_prop(p, indent)).collect()
    }

    /// Serialize this override's description, prefixed with `indent`.
    /// `None` when there is nothing beyond the first line.
    pub fn fmt_desc_block(&self, indent: &str) -> Option<Vec<String>> {
        let rest = self.description.as_ref()?.get_split(true).1?;
        let body_indent = format!("{indent}  ");
        Some(rest.split('\n').map(|l| format!("{body_indent}{l}")).collect())
    }
}

/// Index of the register to override. None if register is not an array or to override all registers
pub type OptArrayIndex = Option<u16>;
/// Field name of the register to override. None if override is for the register it-self
pub type FieldOverrideIndex = Option<String>;

#[derive(Debug, Clone, Default)]
/// Override index info: List of register indexes, optional field name and field array indexes
pub struct OverrideIndex(Vec<u16>, FieldOverrideIndex, Vec<u16>);

impl OverrideIndex {
    pub fn clear(&mut self) {
        self.0.clear();
        self.1 = None;
        self.2.clear();
    }

    /// True when this index targets the whole register
    pub fn is_whole_reg(&self) -> bool {
        self.0.is_empty()
    }

    /// True when this index targets a single field with no array index
    pub fn is_single_field(&self) -> bool {
        self.0.is_empty() && self.2.is_empty()
    }

    /// Field name this index targets, if any (`None` for a register-level override).
    pub fn field_name(&self) -> Option<&str> {
        self.1.as_deref()
    }

    pub fn set_reg_list(&mut self, reg_list: Vec<u16>) {
        self.0 = reg_list;
        self.1 = None;
        self.2.clear();
    }

    pub fn set_field_name(&mut self, name: String) {
        self.1 = Some(name);
    }

    pub fn set_field_list(&mut self, name: String, field_list: Vec<u16>) {
        self.1 = Some(name);
        self.2 = field_list;
    }

    pub fn iter_reg(&self) -> OverrideIndexIter<'_> {
        OverrideIndexIter {data: &self.0, idx: 0}
    }

    pub fn iter_field(&self) -> OverrideIndexIter<'_> {
        OverrideIndexIter {data: &self.2, idx: 0}
    }
}

pub struct OverrideIndexIter<'a> {
    data: &'a[u16],
    idx: usize
}

impl<'a> Iterator for OverrideIndexIter<'a> {
    type Item = OptArrayIndex;

    fn next(&mut self) -> Option<Self::Item> {
        if self.data.is_empty() && self.idx == 0 {
            self.idx += 1;
            Some(None)
        } else {
            let out = self.data.get(self.idx);
            self.idx += 1;
            out.map(|d| Some(*d))
        }
    }

}

pub type RegOverrideDict = HashMap<OptArrayIndex,RegOverride>;

/// Tuple from parser
/// Values are: instance name, array size, type name, group name, addressing scheme and address
pub type RegInstTuple<'a> = (&'a str, ExprTokens, Option<&'a str>, Option<&'a str>, Option<Address>);

#[derive(Clone, Debug, PartialEq)]
/// Register instance description
pub struct RegInst {
    /// Name of the register instance
    pub inst_name: String,
    /// Name of the register type
    pub type_name: String,
    /// Name of the hardware structure the register is part of (only needed when the structure spans multiple registers)
    pub group_name: String,
    /// Address of the instance
    pub addr: Address,
    /// Number of the instance: can be an integer, a parameter or a generic
    pub array: ExprTokens,
    /// Register settings override
    pub reg_override : RegOverrideDict,
    /// Source-tracking metadata for round-tripping edits back to the `.rif` file.
    pub src: DeclLine,
}

impl From<RegInstTuple<'_>> for RegInst {
    fn from(info:RegInstTuple) -> RegInst {
        let default_group = if info.2.is_some() {info.0} else {""};
        RegInst {
            inst_name: info.0.to_owned(),
            type_name: info.2.unwrap_or(info.0).to_owned(),
            group_name:info.3.unwrap_or(default_group).to_owned(),
            addr: info.4.unwrap_or_default(),
            array: info.1,
            reg_override: HashMap::new(),
            src: DeclLine::default(),
        }
    }
}

impl RegInst {

    /// Serialize the instance's *declaration line* back to `.rif` syntax, prefixed with `indent`.
    /// Produces `{indent}- inst_name = type_name (group_name) @ address`
    pub fn fmt_decl(&self, indent: &str) -> Option<String> {
        let mut s = String::with_capacity(indent.len() + self.inst_name.len() + 16);
        s.push_str(indent);
        s.push_str("- ");
        s.push_str(&self.inst_name);
        if !self.array.is_empty() {
            s.push('[');
            s.push_str(&self.array.to_rif());
            s.push(']');
        }
        let has_explicit_type = self.type_name != self.inst_name;
        if has_explicit_type {
            s.push_str(" = ");
            s.push_str(&self.type_name);
        }
        let default_group = if has_explicit_type { self.inst_name.as_str() } else { "" };
        if self.group_name != default_group {
            s.push_str(" (");
            s.push_str(&self.group_name);
            s.push(')');
        }
        if self.addr != Address::default() {
            s.push(' ');
            s.push_str(&self.addr.to_rif());
        }
        Some(s)
    }

    /// Return mutable access to a field override setting
    fn get_field_ovr<'a>(reg: &'a mut RegOverride, name: &str, idx: OptArrayIndex) -> &'a mut FieldOverride{
        let mut field_name = name.to_owned();
        if let Some(field_idx) = idx {
            field_name.push_str(&format!("[{field_idx}]"));
        }
        reg.fields.entry(field_name).or_default()
    }

    /// Return reference to a register override, given an array index
    pub fn get_ovr(&self, idx: &ArrayIdx) -> Option<&RegOverride> {
        self.reg_override.get(&idx.opt_idx())
            .or_else(|| self.reg_override.get(&None))
    }

    /// Stamp the source line of a register-level override property
    pub fn stamp_prop(&mut self, idx: &OverrideIndex, prop: RegOverrideProp, line: usize) {
        for reg_idx in idx.iter_reg() {
            self.reg_override.entry(reg_idx).or_default().src.prop_lines.insert(prop, line);
        }
    }

    /// Stamp the source line of a field-override property
    pub fn stamp_field_prop(&mut self, idx: &OverrideIndex, prop: FieldOverrideProp, line: usize) {
        let Some(name) = idx.field_name() else { return };
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            for field_idx in idx.iter_field() {
                Self::get_field_ovr(reg, name, field_idx).src.prop_lines.insert(prop, line);
            }
        }
    }

    /// Stamp the source line range of a register-level description block
    pub fn stamp_desc_block(&mut self, idx: &OverrideIndex, key: DescBlockKind, block_range: (usize, usize)) {
        for reg_idx in idx.iter_reg() {
            self.reg_override.entry(reg_idx).or_default().src.desc_blocks.insert(key, block_range);
        }
    }

    /// Stamp the source line range of a field-override description
    pub fn stamp_field_desc_block(&mut self, idx: &OverrideIndex, key: DescBlockKind, block_range: (usize, usize)) {
        let Some(name) = idx.field_name() else { return };
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            for field_idx in idx.iter_field() {
                Self::get_field_ovr(reg, name, field_idx).src.desc_blocks.insert(key, block_range);
            }
        }
    }

    /// Override description of a register/field
    pub fn desc_updt(&mut self, idx: &OverrideIndex, desc: &str, is_private: bool) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            match &idx.1 {
                Some(name) => {
                    for field_idx in idx.iter_field() {
                        let field = Self::get_field_ovr(reg, name, field_idx);
                        field.description.get_or_insert_with(Description::default).updt(desc, is_private);
                    }
                }
                None => {
                    reg.description.get_or_insert_with(Description::default).updt(desc, is_private);
                }
            }
        }
    }

    /// Set optional condition in override settings
    pub fn set_optional(&mut self, idx: &OverrideIndex, v: ExprTokens) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            match &idx.1 {
                Some(name) => {
                    for field_idx in idx.iter_field() {
                        let field = Self::get_field_ovr(reg, name, field_idx);
                        field.optional = v.clone();
                    }
                },
                None => reg.optional = v.clone(),
            }
        }
    }

    /// Set optional access in override settings
    pub fn set_addr(&mut self, idx: &OverrideIndex, addr: Address) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            reg.addr = Some(addr.clone());
        }
    }

    /// Set optional access in override settings
    pub fn set_optional_acc(&mut self, idx: &OverrideIndex, acc: Access) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            reg.optional_acc = Some(acc);
        }
    }

    /// Override a register/field visibility
    pub fn set_visibility(&mut self, idx: &OverrideIndex, v: Visibility) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            match &idx.1 {
                Some(name) => {
                    for field_idx in idx.iter_field() {
                        let field = Self::get_field_ovr(reg, name, field_idx);
                        field.visibility = Some(v);
                    }
                },
                None => reg.visibility = Some(v),
            }
        }
    }

    /// Override register hardware access
    pub fn set_hw_acc(&mut self, idx: &OverrideIndex, v: Access) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            reg.hw_acc = Some(v);
        }
    }

    /// Override reset valye of register/field
    pub fn set_reset(&mut self, idx: &OverrideIndex, v: ResetValP) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            if let Some(name) = &idx.1 {
                for field_idx in idx.iter_field() {
                    let field = Self::get_field_ovr(reg, name, field_idx);
                    field.reset = ResetValOverride::Reset(v.clone());
                }
            }
        }
    }

    /// Override limit of a field
    pub fn set_limit(&mut self, idx: &OverrideIndex, limit: LimitP) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            if let Some(name) = &idx.1 {
                for field_idx in idx.iter_field() {
                    let field = Self::get_field_ovr(reg, name, field_idx);
                    field.limit = Some(limit.clone());
                }
            }
        }
    }

    /// Override info (key/value pair) for a register/field
    pub fn add_info(&mut self, idx: &OverrideIndex, key_val:(&str, &str)) {
        for reg_idx in idx.iter_reg() {
            let reg = self.reg_override.entry(reg_idx).or_default();
            match &idx.1 {
                Some(name) => {
                    for field_idx in idx.iter_field() {
                        let field = Self::get_field_ovr(reg, name, field_idx);
                        field.info.insert(key_val.0.to_owned(), key_val.1.to_owned());
                    }
                },
                None => {panic!("Unsuported info on reg");},
            }
        }
    }

}
