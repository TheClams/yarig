use std::collections::HashMap;

use crate::{error::RifErrorKind, parser::parser_expr::ParamValues};

use super::{Access, ClkEn, Context, Description, Field, FieldSwKind, InterruptInfo, InterruptInfoField, Visibility, Width};

#[allow(dead_code)]
#[derive(Clone, Debug, PartialEq)]
pub enum RegPulseKind {
    Write(String),
    Read(String),
    Access(String),
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

#[derive(Clone, Debug, PartialEq, Default)]
pub struct RegDef {
    pub name: String,
    pub group: RegGroup,
    pub description: Description,
    pub pulse: Vec<RegPulseKind>,
    pub fields: Vec<Field>,
    pub interrupt: Vec<InterruptInfo>,
    pub visibility: Visibility,
    pub clk: Option<String>,
    pub rst: Option<String>,
    pub clk_en: ClkEn,
    pub clear: Option<String>,
    pub info: HashMap<String, String>,
    pub array: Width,
    /// Indicates if the register logic is internal, fully external or just for the register access done
    pub external: ExternalKind,
    /// Indicates the register instance is controlled by a parameter
    pub optional: String,
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
            ..Default::default()
        }
    }

    pub fn desc_intr_updt(&mut self, cntxt: &Context, name: &str, desc: &str) -> Result<(), RifErrorKind> {
        let intr = self.interrupt.iter_mut().find(|intr| intr.name==*name).ok_or(RifErrorKind::NotIntr)?;
        let d = match cntxt {
            Context::DescIntrEnable => &mut intr.description.enable,
            Context::DescIntrMask => &mut intr.description.mask,
            Context::DescIntrPending => &mut intr.description.pending,
            _ => unreachable!(),
        };
        d.updt(desc);
        Ok(())
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
        if !self.optional.is_empty() {
            if let Some(&x) = params.get(&self.optional) {
                x==0
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get the register group name
    pub fn get_group_name(&self) -> &str {
        &self.group.name
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