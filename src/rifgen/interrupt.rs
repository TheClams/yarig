use super::{ResetVal, Description};

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum InterruptTrigger {#[default] High, Low, Rising, Falling, Edge}

impl InterruptTrigger {
    pub fn is_level(&self) -> bool {
        matches!(self,InterruptTrigger::High | InterruptTrigger::Low)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum InterruptRegKind {#[default] None, Base, Enable, Mask, Pending}
impl InterruptRegKind {
    /// Return True when the kind is enable/mask/pending
    pub fn is_derived(&self) -> bool {
        self==&InterruptRegKind::Enable ||
        self==&InterruptRegKind::Mask ||
        self==&InterruptRegKind::Pending
    }

    pub fn is_base(&self) -> bool {
        self==&InterruptRegKind::Base
    }

    pub fn is_pending(&self) -> bool {
        self==&InterruptRegKind::Pending
    }

    pub fn get_suffix(&self) -> &str {
        match self {
            InterruptRegKind::Enable  => "_en",
            InterruptRegKind::Mask    => "_mask",
            InterruptRegKind::Pending => "_pending",
            _ => "",
        }
    }
}


#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum InterruptClr {#[default] Read, Write0, Write1, Hw}

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
    pub enable: Option<ResetVal>,
    pub mask: Option<ResetVal>,
    pub pending: bool,
    pub description: InterruptDesc,
}

pub type InterruptPropTuple = (Option<InterruptTrigger>,Option<InterruptClr>,Option<ResetVal>,Option<ResetVal>,Option<bool>);
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

    pub fn get_rst_desc(&self, kind: InterruptRegKind) -> Option<(&ResetVal,&Description)> {
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
                Some((&ResetVal::Unsigned(0),&self.description.pending))
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
