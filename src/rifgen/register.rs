use std::collections::HashMap;

use crate::{error::RifErrorKind, parser::parser_expr::ParamValues};
use crate::hdl::LogicExpr;

use super::{Access, ClkEn, Context, Description, DescBlockKind, Field, FieldSwKind, InterruptInfo, InterruptInfoField, InterruptRegKind, PropBlocks, PropLines, Visibility, Width};

#[derive(Clone, Debug, PartialEq)]
pub enum RegPulseKind {
    Write(String),
    Read(String),
    Access(String),
}

impl RegPulseKind {
    pub fn clk(&self) -> &str {
        match self {
            RegPulseKind::Write(n)  => n,
            RegPulseKind::Read(n)   => n,
            RegPulseKind::Access(n) => n,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ExternalKind {#[default]
    None,
    ReadWrite,
    Read,
    Write,
    Done,
}

impl ExternalKind {
    /// Return a new externalKind taking into account the port access
    pub fn with_access(&self, access: &Access) -> ExternalKind {
        match self {
            ExternalKind::ReadWrite => {
                match access {
                    Access::RO => ExternalKind::Read,
                    Access::WO => ExternalKind::Write,
                    Access::RW => ExternalKind::ReadWrite,
                    Access::NA => ExternalKind::None,
                }
            }
            _ => *self
        }
    }

    /// Flag External Read/Write access
    pub fn is_rw(&self) -> bool {
        matches!(self,
            ExternalKind::Read |
            ExternalKind::Write |
            ExternalKind::ReadWrite)
    }

    /// Flag External Read/Write access
    pub fn is_none(&self) -> bool {
        *self==ExternalKind::None
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RegGroup {
    pub name: String,
    pub pkg: Option<String>
}

impl RegGroup {
    pub fn new(name: &str, pkg: Option<&str>) -> Self {
        RegGroup {
            name: name.to_owned(),
            pkg: pkg.map(|n| n.to_owned())
        }
    }
}

impl From<(Option<&str>, &str)> for RegGroup {
    fn from(value: (Option<&str>, &str)) -> Self {
        RegGroup::new(value.1, value.0)
    }
}

/// Register property setting (used to track source metadata to support edition)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RegProp {
    Visibility,
    Clock,
    Reset,
    External,
    WrPulse,
    RdPulse,
    AccPulse,
    Interrupt,
    InterruptAlt,
}

impl RegProp {
    /// All managed sub-properties, in the canonical order they are emitted for a new register.
    pub const ALL: [RegProp; 9] = [
        RegProp::Interrupt,
        RegProp::InterruptAlt,
        RegProp::Clock,
        RegProp::Reset,
        RegProp::External,
        RegProp::WrPulse,
        RegProp::RdPulse,
        RegProp::AccPulse,
        RegProp::Visibility,
    ];
}

/// Source-tracking metadata attached to a parsed `RegDef`, used by editors to round-trip
/// edits back to the `.rif` with minimal disturbance to the file. Same shape as `Field`'s
/// `SrcInfo`, parameterized on `RegProp` instead of `FieldProp`.
#[derive(Clone, Debug, Default)]
pub struct RegSrcInfo {
    /// 1-based line number of the declaration in its source file.
    /// `None` when the register was created programmatically rather than parsed.
    pub decl_line: Option<usize>,
    /// Whether the source declaration line carried an inline `"description"`.
    /// Used to round-trip the declaration line without inventing a `description:` block.
    pub has_inline_desc: bool,
    /// 1-based source line of each managed sub-property present at parse time.
    pub prop_lines: PropLines<RegProp>,
    /// 1-based (start, end) source lines of each tracked description-style block: public/private
    /// `description:`, and the primary interrupt's `enable`/`mask`/`pending.description:`
    /// (public-only — no private/hidden variant is exposed for editing there).
    pub desc_blocks: PropBlocks<DescBlockKind>,
}

impl RegSrcInfo {
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

/// Source position never affects equality
impl PartialEq for RegSrcInfo {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// Register deifnition: name, description, clk/reset, list of fields, ...
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RegDef {
    /// Register type name
    pub name: String,
    /// Register group name: all register in a group are merged together at hardware level
    pub group: RegGroup,
    /// Register description
    pub description: Description,
    /// Defines hardware pulse generated on access (read/write/both)
    pub pulse: Vec<RegPulseKind>,
    /// List of fields
    pub fields: Vec<Field>,
    /// Interrupt definition
    pub interrupt: Vec<InterruptInfo>,
    /// Visibility of register: hidden removes register from doc, reserved force name to rsvd_xx in doc and disabled force register to read-only
    pub visibility: Visibility,
    /// Optional clock (automatically chosen between software  and first hardware if not define)
    pub clk: Option<String>,
    /// Optional reset (default to page/rif if not defined)
    pub rst: Option<String>,
    /// Clock Enable definition
    pub clk_en: ClkEn,
    /// Optional clear logic expression
    pub clear: Option<LogicExpr>,
    /// Generic key/value pair to provide extra information to generators
    pub info: HashMap<String, String>,
    /// Width of the array (fixed or from parameter). Null when register is not an array
    pub array: Width,
    /// Indicates if the register logic is internal, fully external or just for the register access done
    pub external: ExternalKind,
    /// Indicates the register instance is controlled by a parameter
    pub optional: String,
    /// Register access when optional and disabled: controls if an error is raised when a disabled register is accessed
    pub optional_acc: Access,
    /// Source-tracking metadata for edition
    pub src: RegSrcInfo,
}

impl RegDef {
    pub fn new(name: &str, group: Option<(Option<&str>,&str)>, array: Option<Width>, desc: &str) -> Self {
        let group : RegGroup = match group {
            Some(pair) => pair.into(),
            None => RegGroup::new(name, None),
        };
        RegDef {
            name: name.to_owned(),
            group,
            array: array.unwrap_or_default(),
            description: desc.into(),
            optional_acc: Access::NA,
            ..Default::default()
        }
    }

    pub fn desc_intr_updt(&mut self, cntxt: &Context, name: &str, desc: &str, is_private: bool) -> Result<(), RifErrorKind> {
        let intr = self.interrupt.iter_mut().find(|intr| intr.name==*name).ok_or(RifErrorKind::NotIntr)?;
        let d = match cntxt {
            Context::DescIntrEnable => &mut intr.description.enable,
            Context::DescIntrMask => &mut intr.description.mask,
            Context::DescIntrPending => &mut intr.description.pending,
            _ => unreachable!(),
        };
        d.updt(desc, is_private);
        Ok(())
    }

    /// Mutable access to the primary interrupt's description for a derived kind.
    pub fn intr_desc_mut(&mut self, kind: InterruptRegKind) -> Option<&mut Description> {
        let intr = self.interrupt.first_mut()?;
        Some(match kind {
            InterruptRegKind::Enable  => &mut intr.description.enable,
            InterruptRegKind::Mask    => &mut intr.description.mask,
            InterruptRegKind::Pending => &mut intr.description.pending,
            _ => return None,
        })
    }

    /// Re-apply the primary interrupt's current trigger/clear to every field that does not carry its own override.
    /// Needed to support edition of register/fields properties
    pub fn refresh_intr_defaults(&mut self) {
        let Some(intr) = self.interrupt.first() else { return };
        let (trigger, clear) = (intr.trigger, intr.clear);
        for f in self.fields.iter_mut() {
            if f.intr_ovr.trigger.is_none() && f.intr_ovr.clear.is_none() {
                f.set_intr(InterruptInfoField { trigger: Some(trigger), clear: Some(clear) });
            }
        }
    }

    // Add field to a register
    // And automatically set the Hw/Sw kinds when register is an interrupt
    pub fn add_field(&mut self, mut f: Field) {
        if let Some(intr) = &self.interrupt.first() {
            f.set_intr(InterruptInfoField {
                trigger: Some(intr.trigger),
                clear: Some(intr.clear),
            });
        }
        // Update external kind to differentiate the different kind based on field access
        if self.fields.is_empty() && self.external==ExternalKind::ReadWrite {
            match f.sw_kind {
                FieldSwKind::ReadClr |
                FieldSwKind::ReadOnly => self.external = ExternalKind::Read,
                FieldSwKind::WriteOnly => self.external = ExternalKind::Write,
                _ => {}
            }
        } else if self.external==ExternalKind::Read {
            match f.sw_kind {
                FieldSwKind::WriteOnly |
                FieldSwKind::W1Clr |
                FieldSwKind::W0Clr |
                FieldSwKind::W1Set |
                FieldSwKind::W1Tgl |
                FieldSwKind::W1Pulse(_, _) |
                FieldSwKind::Password(_) => self.external = ExternalKind::ReadWrite,
                _ => {}
            }
        } else if self.external==ExternalKind::Write {
            match f.sw_kind {
                FieldSwKind::ReadClr |
                FieldSwKind::ReadOnly |
                FieldSwKind::ReadWrite => self.external = ExternalKind::ReadWrite,
                _ => {}
            }

        }
        self.fields.push(f);
    }

    /// Add Generic information Key/Value
    pub fn add_info(&mut self, key_val: (&str, &str)) {
        self.info.insert(key_val.0.to_owned(), key_val.1.to_owned());
    }

    /// Set visibility to hidden
    pub fn hidden(&mut self) {
        self.visibility = Visibility::Hidden;
    }

    /// Set visibility to reserved
    pub fn reserved(&mut self) {
        self.visibility = Visibility::Reserved;
    }

    /// High when register instance is deactivated
    pub fn ignored(&self, params: &ParamValues) -> bool {
        !self.optional.is_empty() && params.get(&self.optional).is_some_and(|&x| x == 0)
    }

    /// Get the register group name
    pub fn get_group_name(&self) -> &str {
        &self.group.name
    }

    /// Check if register can be written by software
    pub fn is_sw_wr(&self) -> bool {
        self.fields.iter().any(|f| !f.sw_kind.is_ro())
    }

    /// Serialize the register's *declaration line* back to `.rif` syntax, prefixed with `indent`.
    /// Produces `{indent}- name[array] : (group) "description"`
    pub fn fmt_decl(&self, indent: &str) -> String {
        let mut s = String::with_capacity(indent.len() + self.name.len() + 16);
        s.push_str(indent);
        s.push_str("- ");
        s.push_str(&self.name);
        if !matches!(self.array, Width::Value(0)) {
            s.push('[');
            s.push_str(&self.array.to_string());
            s.push(']');
        }
        s.push_str(" :");
        let is_default_group = self.group.pkg.is_none() && self.group.name == self.name;
        if !is_default_group {
            s.push_str(" (");
            if let Some(pkg) = &self.group.pkg {
                s.push_str(pkg);
                s.push_str("::");
            }
            s.push_str(&self.group.name);
            s.push(')');
        }
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

    /// Serialize a property as its canonical `.rif` line, prefixed with `indent`.
    pub fn fmt_prop(&self, prop: RegProp, indent: &str) -> Option<String> {
        match prop {
            RegProp::Visibility => match self.visibility {
                Visibility::Hidden => Some(format!("{indent}hidden")),
                Visibility::Reserved => Some(format!("{indent}reserved")),
                Visibility::Full | Visibility::Disabled | Visibility::Unused => None,
            },
            RegProp::Clock => self.clk.as_ref().map(|c| format!("{indent}clock {c}")),
            RegProp::Reset => self.rst.as_ref().map(|r| format!("{indent}hwReset {r}")),
            RegProp::External => match self.external {
                ExternalKind::ReadWrite => Some(format!("{indent}external")),
                ExternalKind::Done => Some(format!("{indent}externalDone")),
                ExternalKind::None | ExternalKind::Read | ExternalKind::Write => None,
            },
            RegProp::Interrupt => {
                let intr = self.interrupt.first()?;
                let mut body = format!("interrupt {} {}", intr.trigger.to_rif(), intr.clear.to_rif());
                if let Some(en) = &intr.enable {
                    body.push_str(&format!(" en={}", en.to_rif()));
                }
                if let Some(mask) = &intr.mask {
                    body.push_str(&format!(" mask={}", mask.to_rif()));
                }
                if intr.pending {
                    body.push_str(" pending");
                }
                Some(format!("{indent}{body}"))
            }
            RegProp::InterruptAlt => {
                let alt = self.interrupt.get(1)?;
                let mut body = format!("alt {} {} {}", alt.name, alt.trigger.to_rif(), alt.clear.to_rif());
                if let Some(en) = &alt.enable {
                    body.push_str(&format!(" en={}", en.to_rif()));
                }
                if let Some(mask) = &alt.mask {
                    body.push_str(&format!(" mask={}", mask.to_rif()));
                }
                if alt.pending {
                    body.push_str(" pending");
                }
                Some(format!("{indent}{body}"))
            }
            RegProp::WrPulse | RegProp::RdPulse | RegProp::AccPulse => {
                let (keyword, clk) = match prop {
                    RegProp::WrPulse => ("wrPulse", self.pulse.iter().find_map(|p| if let RegPulseKind::Write(c) = p { Some(c) } else { None })?),
                    RegProp::RdPulse => ("rdPulse", self.pulse.iter().find_map(|p| if let RegPulseKind::Read(c) = p { Some(c) } else { None })?),
                    RegProp::AccPulse => ("accPulse", self.pulse.iter().find_map(|p| if let RegPulseKind::Access(c) = p { Some(c) } else { None })?),
                    _ => unreachable!("handled above"),
                };
                let kind = if clk.is_empty() { "comb" } else { "reg" };
                Some(format!("{indent}{keyword} {kind}"))
            }
        }
    }

    /// Serialized all properties
    pub fn fmt_prop_all(&self, indent: &str) -> Vec<String> {
        RegProp::ALL.iter().filter_map(|&p| self.fmt_prop(p, indent)).collect()
    }

    /// Serialize the register's public `description:`
    pub fn fmt_desc_block(&self, indent: &str) -> Option<Vec<String>> {
        let rest = self.description.get_split(true).1?;
        let body_indent = format!("{indent}  ");
        let mut lines = vec![format!("{indent}description:")];
        lines.extend(rest.split('\n').map(|l| format!("{body_indent}{l}")));
        Some(lines)
    }

    /// Serialize the primary interrupt's `{enable,mask,pending}.description:` block.
    pub fn fmt_intr_desc_block(&self, kind: InterruptRegKind, indent: &str) -> Option<Vec<String>> {
        let intr = self.interrupt.first()?;
        let desc = match kind {
            InterruptRegKind::Enable  => &intr.description.enable,
            InterruptRegKind::Mask    => &intr.description.mask,
            InterruptRegKind::Pending => &intr.description.pending,
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

}

#[derive(Clone, Debug, PartialEq)]
pub enum RegDefOrIncl {
    Include(String),
    Def(Box<RegDef>),
}

impl RegDefOrIncl {

    pub fn get_regdef_mut(&mut self) -> Option<&mut RegDef> {
        match self {
            RegDefOrIncl::Def(d) => Some(d),
            _ => None,
        }
    }

    pub fn get_regdef(&self) -> Option<&RegDef> {
        match self {
            RegDefOrIncl::Def(d) => Some(d),
            _ => None,
        }
    }

    pub fn get_inc(&self) -> Option<&str> {
        match self {
            RegDefOrIncl::Include(d) => Some(d),
            _ => None,
        }
    }

    pub fn get_name(&self) -> &str {
        match self {
            RegDefOrIncl::Include(inc) => inc.split('.').collect::<Vec<&str>>().get(3).unwrap_or(&"*"),
            RegDefOrIncl::Def(def) => &def.name,
        }
    }

}

#[derive(Debug)]
pub struct RegIncludePath<'a> {
    pub rif : &'a str,
    pub page: &'a str,
    pub reg : &'a str,
}

impl<'a> RegIncludePath<'a> {

    pub fn new(inc: &'a str) -> Result<Self,String> {
        let mut s = inc.split('.');
        let (Some(rif),Some(page),Some(reg)) = (s.next(),s.next(),s.next()) else {
            return Err(format!("Register include format is: rif.page.register : got {inc} !"));
        };
        Ok(RegIncludePath {rif,page,reg})
    }
}