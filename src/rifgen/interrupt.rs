use super::{ResetValP, Description};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum InterruptTrigger {#[default] High, Low, Rising, Falling, Edge}

impl InterruptTrigger {
    pub fn is_level(&self) -> bool {
        matches!(self,InterruptTrigger::High | InterruptTrigger::Low)
    }

    /// Canonical `.rif` keyword for this trigger.
    pub fn to_rif(&self) -> &'static str {
        match self {
            InterruptTrigger::High    => "high",
            InterruptTrigger::Low     => "low",
            InterruptTrigger::Rising  => "rising",
            InterruptTrigger::Falling => "falling",
            InterruptTrigger::Edge    => "edge",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum InterruptRegKind {#[default] None, Base, Enable, Mask, Pending}
impl InterruptRegKind {
    /// Return True when the kind is enable/mask/pending
    pub fn is_derived(&self) -> bool {
        self==&InterruptRegKind::Enable ||
        self==&InterruptRegKind::Mask ||
        self==&InterruptRegKind::Pending
    }

    pub fn is_none(&self) -> bool {
        self==&InterruptRegKind::None
    }

    pub fn is_base(&self) -> bool {
        self==&InterruptRegKind::Base
    }

    pub fn is_pending(&self) -> bool {
        self==&InterruptRegKind::Pending
    }

    /// Suffix (with its leading underscore) identifying a derived interrupt register in RTL signals and type names.
    /// Always empty for the status register since it shares the name of the register definition.
    pub fn get_suffix(&self, sfx: &InterruptSuffixes) -> String {
        match self {
            InterruptRegKind::Enable  => sfx.enable_suffix(),
            InterruptRegKind::Mask    => sfx.mask_suffix(),
            InterruptRegKind::Pending => sfx.pending_suffix(),
            _ => String::new(),
        }
    }
}

/// Suffixes used to name the registers generated from an interrupt definition:
/// `<name><status>` for the status register, `<name>_<enable>`, `<name>_<mask>` and `<name>_<pending>` for the optional ones.
/// Values are stored without the leading underscore, which is added automatically when the value is not empty.
#[derive(Clone, Debug, PartialEq, Eq, serde_derive::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InterruptSuffixes {
    /// Suffix of the status register (empty by default: the register keeps the name of its definition)
    pub status: String,
    /// Suffix of the enable register
    pub enable: String,
    /// Suffix of the mask register
    pub mask: String,
    /// Suffix of the pending register
    pub pending: String,
}

impl Default for InterruptSuffixes {
    fn default() -> Self {
        InterruptSuffixes {
            status: String::new(),
            enable: "en".to_owned(),
            mask: "mask".to_owned(),
            pending: "pending".to_owned(),
        }
    }
}

impl InterruptSuffixes {

    fn fmt_suffix(s: &str) -> String {
        if s.is_empty() {String::new()} else {format!("_{s}")}
    }

    /// Suffix for the status register (including leading underscore, empty when not set)
    pub fn status_suffix(&self) -> String {Self::fmt_suffix(&self.status)}
    /// Suffix for the enable register (including leading underscore)
    pub fn enable_suffix(&self) -> String {Self::fmt_suffix(&self.enable)}
    /// Suffix for the mask register (including leading underscore)
    pub fn mask_suffix(&self) -> String {Self::fmt_suffix(&self.mask)}
    /// Suffix for the pending register (including leading underscore)
    pub fn pending_suffix(&self) -> String {Self::fmt_suffix(&self.pending)}

    /// Suffix added to the register instance name by the automatic instantiation:
    /// like [`InterruptRegKind::get_suffix`] but also applies the status suffix to the base register
    pub fn inst_suffix(&self, kind: InterruptRegKind) -> String {
        match kind {
            InterruptRegKind::Base => self.status_suffix(),
            _ => kind.get_suffix(self),
        }
    }

    /// Check suffixes can be used to build valid and distinct register names
    pub fn validate(&self) -> Result<(), String> {
        let items = [
            ("status", &self.status),
            ("enable", &self.enable),
            ("mask", &self.mask),
            ("pending", &self.pending),
        ];
        for (name, value) in items {
            if value.starts_with('_') {
                return Err(format!("Interrupt {name} suffix '{value}' should not start with an underscore (it is added automatically)"));
            }
            if let Some(c) = value.chars().find(|c| !c.is_ascii_alphanumeric() && *c != '_') {
                return Err(format!("Interrupt {name} suffix '{value}' contains illegal character '{c}'"));
            }
            if name != "status" && value.is_empty() {
                return Err(format!("Interrupt {name} suffix cannot be empty"));
            }
        }
        for (i, (n0, v0)) in items.iter().enumerate() {
            if let Some((n1, _)) = items.iter().skip(i + 1).find(|(_, v1)| v0 == v1) {
                return Err(format!("Interrupt {n0} and {n1} suffixes must be different (both are '{v0}')"));
            }
        }
        Ok(())
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum InterruptClr {#[default] Read, Write0, Write1, Hw}

impl InterruptClr {
    /// Canonical `.rif` keyword for this clear method.
    pub fn to_rif(&self) -> &'static str {
        match self {
            InterruptClr::Read   => "rclr",
            InterruptClr::Write0 => "w0clr",
            InterruptClr::Write1 => "w1clr",
            InterruptClr::Hw     => "hwclr",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct InterruptDesc {
    pub enable: Description,
    pub mask: Description,
    pub pending: Description,
}


#[derive(Clone, Debug, PartialEq)]
pub struct InterruptInfo {
    pub name: String,
    pub trigger: InterruptTrigger,
    pub clear: InterruptClr,
    pub enable: Option<ResetValP>,
    pub mask: Option<ResetValP>,
    pub pending: bool,
    pub description: InterruptDesc,
}

pub type InterruptPropTuple = (Option<InterruptTrigger>,Option<InterruptClr>,Option<ResetValP>,Option<ResetValP>,Option<bool>);
impl InterruptInfo {
    pub fn new(name: &str, info: InterruptPropTuple) -> InterruptInfo {
        InterruptInfo {
            name: name.to_owned(),
            trigger: info.0.unwrap_or_default(),
            clear: info.1.unwrap_or_default(),
            enable: info.2,
            mask: info.3,
            pending: info.4.unwrap_or(false),
            description : InterruptDesc::default(),
        }
    }

    pub fn get_rst_desc(&self, kind: InterruptRegKind) -> Option<(&ResetValP,&Description)> {
        match kind {
            InterruptRegKind::Enable => {
                let rst = self.enable.as_ref()?;
                Some((rst,&self.description.enable))
            },
            InterruptRegKind::Mask => {
                let rst = self.mask.as_ref()?;
                Some((rst,&self.description.mask))
            },
            InterruptRegKind::Pending if self.pending => {
                Some((&ResetValP::Unsigned(0),&self.description.pending))
            },
            _ => None
        }
    }

    /// True when trigger requires a register
    pub fn edge_trigger(&self) -> bool {
        matches!(self.trigger, InterruptTrigger::Rising | InterruptTrigger::Falling | InterruptTrigger::Edge)
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct InterruptInfoField {
    pub trigger: Option<InterruptTrigger>,
    pub clear: Option<InterruptClr>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_suffixes() {
        let sfx = InterruptSuffixes::default();
        assert!(sfx.validate().is_ok());
        assert_eq!(InterruptRegKind::Base.get_suffix(&sfx), "");
        assert_eq!(InterruptRegKind::Enable.get_suffix(&sfx), "_en");
        assert_eq!(InterruptRegKind::Mask.get_suffix(&sfx), "_mask");
        assert_eq!(InterruptRegKind::Pending.get_suffix(&sfx), "_pending");
        assert_eq!(InterruptRegKind::Base.get_suffix(&sfx), sfx.inst_suffix(InterruptRegKind::Base));
    }

    #[test]
    fn custom_suffixes() {
        let sfx = InterruptSuffixes {status: "st".to_owned(), mask: "msk".to_owned(), ..Default::default()};
        assert!(sfx.validate().is_ok());
        // The status suffix applies to the instance name only
        assert_eq!(sfx.inst_suffix(InterruptRegKind::Base), "_st");
        assert_eq!(InterruptRegKind::Base.get_suffix(&sfx), "");
        assert_eq!(sfx.inst_suffix(InterruptRegKind::Mask), "_msk");
        assert_eq!(InterruptRegKind::Mask.get_suffix(&sfx), "_msk");
        assert_eq!(InterruptRegKind::None.get_suffix(&sfx), "");
    }

    #[test]
    fn invalid_suffixes() {
        let with = |f: fn(&mut InterruptSuffixes)| {
            let mut sfx = InterruptSuffixes::default();
            f(&mut sfx);
            sfx.validate()
        };
        assert!(with(|s| s.mask = "en".to_owned()).is_err(), "duplicate");
        assert!(with(|s| s.status = "mask".to_owned()).is_err(), "duplicate with status");
        assert!(with(|s| s.enable = String::new()).is_err(), "empty enable");
        assert!(with(|s| s.pending = "_p".to_owned()).is_err(), "leading underscore");
        assert!(with(|s| s.pending = "p-q".to_owned()).is_err(), "illegal character");
        assert!(with(|s| s.status = String::new()).is_ok(), "empty status is allowed");
    }

    #[test]
    fn deserialize_partial() {
        let sfx: InterruptSuffixes = toml::from_str("mask = \"msk\"").unwrap();
        assert_eq!(sfx.mask, "msk");
        assert_eq!(sfx.enable, "en");
        assert!(toml::from_str::<InterruptSuffixes>("unknown = \"x\"").is_err());
    }
}
