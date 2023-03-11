use crate::{generator::casing::{Casing, ToCasing}, rifgen::{ClkEn, Interface, Rif, SuffixInfo}};

use super::{comp_inst::RifPageInst, reg_impl::{HwRegs, RegImplDict}};


pub struct SignalInfo {
    pub name: String,
    pub width: u8,
    pub reset: String,
    pub value: String,
    pub enable: Option<String>,
    pub clear: Option<String>,
}

impl SignalInfo {
    pub fn new(name: &str, width: u8, reset: &str, value: &str) -> SignalInfo {
        SignalInfo {
            name: name.to_owned(),
            width,
            reset: reset.to_owned(),
            value: value.to_owned(),
            enable: None,
            clear: None,
        }
    }

    pub fn new_with_en(
        name: &str,
        width: u8,
        reset: &str,
        value: &str,
        enable: &str,
    ) -> SignalInfo {
        SignalInfo {
            name: name.to_owned(),
            width,
            reset: reset.to_owned(),
            value: value.to_owned(),
            enable: if enable.is_empty() {None} else {Some(enable.to_owned())},
            clear: None,
        }
    }

    #[allow(dead_code)]
    pub fn new_with_en_clr(
        name: &str,
        width: u8,
        reset: &str,
        value: &str,
        enable: &str,
        clear: &str,
    ) -> SignalInfo {
        SignalInfo {
            name: name.to_owned(),
            width,
            reset: reset.to_owned(),
            value: value.to_owned(),
            enable: if enable.is_empty() {None} else {Some(enable.to_owned())},
            clear: if clear.is_empty() {None} else {Some(clear.to_owned())},
        }
    }
}

/// Hardware port information: name, width, direction, description
#[derive(Clone, Debug)]
pub struct PortInfo {
    pub name: String,
    pub width: PortWidth,
    pub dir: PortDir,
    pub desc: String,
    pub dim: u16,
}

impl PortInfo {
    pub fn new_in(name: String, desc: String) -> Self {
        PortInfo {
            name,
            width: PortWidth::Basic(1),
            dir: PortDir::In,
            desc,
            dim: 0,
        }
    }

    pub fn new_out(name: String, desc: String) -> Self {
        PortInfo {
            name,
            width: PortWidth::Basic(1),
            dir: PortDir::Out,
            desc,
            dim: 0,
        }
    }

    pub fn new_intf(name: String, if_name: String, desc: String) -> Self {
        PortInfo {
            name,
            width: PortWidth::Custom(if_name),
            dir: PortDir::Modport(("rif".to_owned(), "ctrl".to_owned())),
            desc,
            dim: 0,
        }
    }

    pub fn new(name: String, width: PortWidth, dir: PortDir, desc: String, dim: u16) -> Self {
        PortInfo { name, width, dir, desc, dim}
    }

    pub fn is_intf(&self) -> bool {
        matches!(self.dir, PortDir::Modport(_))
    }

    pub fn width(&self, addr_w: u8, data_w: u8) -> u8 {
        match self.width {
            PortWidth::Basic(w)  => w,
            PortWidth::Address   => addr_w,
            PortWidth::Data      => data_w,
            PortWidth::Custom(_) => 0,
        }
    }
}

/// Describe a port width with distinction between normal signal, address, data
#[derive(Clone, Debug)]
pub enum PortWidth {
    Basic(u8),
    Address,
    Data,
    Custom(String),
}

/// Port Direction: input/Output/ModPort
#[derive(Clone, Debug, PartialEq)]
pub enum PortDir {
    /// Input port
    In,
    /// Output port
    Out,
    /// Defines the names of the two possible modport
    Modport((String,String))
}

impl PortDir {
    pub fn is_in(&self) -> bool {
        self == &PortDir::In
    }
    pub fn is_out(&self) -> bool {
        self == &PortDir::Out
    }
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct PortList {
    pub clocks : Vec<PortInfo>,
    pub resets : Vec<PortInfo>,
    pub clk_ens: Vec<PortInfo>,
    pub ctrls  : Vec<PortInfo>,
    pub regs   : Vec<PortInfo>,
    pub irqs   : Vec<PortInfo>,
    pub pages  : Vec<PortInfo>,
}

#[allow(dead_code)]
impl PortList {
    pub fn new(rif: &Rif, pages_inst: &[RifPageInst], regs_impls: &RegImplDict, hw_regs: &HwRegs, suffix: &Option<SuffixInfo>) -> Self {
        let mut clocks : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut resets : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut clk_ens : Vec<PortInfo> = Vec::with_capacity(2) ;
        let mut ctrls  : Vec<PortInfo> = Vec::with_capacity(2);
        let mut regs   : Vec<PortInfo> = Vec::with_capacity(hw_regs.len());
        let mut irqs   : Vec<PortInfo> = Vec::with_capacity(2) ;
        // Software clocking
        clocks.push(PortInfo::new_in(rif.sw_clocking.clk.to_owned(), "Software clock".to_owned()));
        resets.push(PortInfo::new_in(
            rif.sw_clocking.rst.name.to_owned(),
            format!("Software {}", rif.sw_clocking.rst.desc())
        ));
        if !rif.sw_clocking.en.is_empty() {
            clk_ens.push(PortInfo::new_in(rif.sw_clocking.en.to_owned(), "Software clock enable".to_owned()))
        }
        if !rif.sw_clocking.clear.is_empty() {
            ctrls.push(PortInfo::new_in(
                rif.sw_clocking.clear.to_owned(),
                "Software clear".to_owned()
            ));
        }
        // Hardware Clocking
        for hw_clk in rif.hw_clocking.iter() {
            if !clocks.iter().any(|p| p.name==hw_clk.clk) {
                clocks.push(PortInfo::new_in(hw_clk.clk.to_owned(), "Hardware clock".to_owned()));
            }
            if !resets.iter().any(|p| p.name==hw_clk.rst.name) {
                resets.push(PortInfo::new_in(
                    hw_clk.rst.name.to_owned(),
                    format!("Hardware {}", rif.sw_clocking.rst.desc())));
            }
            if !hw_clk.en.is_empty() && !clk_ens.iter().any(|p| p.name==hw_clk.en) {
                clk_ens.push(PortInfo::new_in(hw_clk.en.to_owned(), "Hardware clock enable".to_owned()));
            }
            if !hw_clk.clear.is_empty() && !ctrls.iter().any(|p| p.name==hw_clk.clear) {
                ctrls.push(PortInfo::new_in(
                    hw_clk.clear.to_owned(),
                    "Hardware clear".to_owned()
                ));
            }
        }
        // Collect Clock enable and controls signals from register implementation
        for r in regs_impls.values() {
            if let ClkEn::Signal(en) = &r.clk_en {
                if !clk_ens.iter().any(|p| &p.name==en) {
                    clk_ens.push(PortInfo::new_in(en.to_owned(), "Clock enable".to_owned()));
                }
            }
            for f in r.fields.iter() {
                if let ClkEn::Signal(en) = &f.clk_en {
                    if !clk_ens.iter().any(|p| &p.name==en) {
                        clk_ens.push(PortInfo::new_in(en.to_owned(), "Clock enable".to_owned()));
                    }
                }
                if let Some(lock) = f.lock.port_name() {
                    if !ctrls.iter().any(|p| p.name==lock) {
                        ctrls.push(PortInfo::new_in(lock.to_owned(), "Lock signal".to_owned()));
                    }
                }
                for k in f.hw_kind.iter() {
                    if let Some(sig) = k.get_signal() {
                        if let Some(sig) = sig.strip_prefix('.') {
                            if !ctrls.iter().any(|p| p.name==sig) {
                                ctrls.push(PortInfo::new_in(sig.to_owned(), "Control signal".to_owned()));
                            }
                        }
                    }
                }
            }
        }
        //
        let pages : Vec<PortInfo> = pages_inst.iter()
            .filter(|p| p.is_external())
            .map(|p| PortInfo::new_intf(
                p.name.to_owned(),
                "rif_if".to_owned(),
                format!("Interface to access register from page {}", p.name)))
            .collect();
        let rif_pkg_name = if let Some(suffix) = suffix {
            if !suffix.pkg  {
                rif.name.to_owned()
            } else if suffix.alt_pos && rif.name.ends_with("_rif") {
                format!("{}_{}_rif", &rif.name[..(rif.name.len()-4)], suffix.name)
            } else {
                format!("{}_{}", rif.name, suffix.name)
            }
        } else {
            rif.name.to_owned()
        };
        let rif_pkg_name = rif_pkg_name.to_casing(Casing::Snake);
        for (group_name, hw_reg) in hw_regs.items() {
            let hw_reg_def = regs_impls.get(&hw_reg.group).unwrap();
            let pkg_name = if let Some(pkg) = &hw_reg_def.pkg {pkg} else {&rif_pkg_name};
            let pkg_name = pkg_name.to_casing(Casing::Snake);
            let group_type = hw_reg.group.to_casing(Casing::Snake);
            let desc = hw_reg_def.description.get_short();
            if hw_reg.port.is_in() {
                let port = PortInfo {
                    name: group_name.to_owned(),
                    width: PortWidth::Custom(format!("{pkg_name}_pkg::t_{group_type}_hw")),
                    dir: PortDir::In,
                    dim: hw_reg.dim,
                    desc: desc.to_owned(),
                };
                regs.push(port);
            }
            if hw_reg.port.is_out() {
                let suffix = if hw_reg.intr_derived {"hw"} else {"sw"};
                let port = PortInfo {
                    name: format!("rif_{group_name}"),
                    width: PortWidth::Custom(format!("{pkg_name}_pkg::t_{group_type}_{suffix}")),
                    dir: PortDir::Out,
                    dim: hw_reg.dim,
                    desc: desc.to_owned(),
                };
                regs.push(port);
            }
            if !hw_reg_def.interrupt.is_empty() && !hw_reg.intr_derived {
                let port = PortInfo::new_out(
                    format!("rif_{group_name}_irq"),
                    format!("High when one interrupt field of {group_name} is asserted"));
                irqs.push(port);
                for info in hw_reg_def.interrupt.iter().skip(1) {
                    let port = PortInfo::new_out(
                        format!("rif_{group_name}_{}_irq", info.name),
                        format!("High when one interrupt field of {group_name}_{} is asserted", info.name));
                    irqs.push(port);
                }
            }
        }
        PortList {
            clocks,
            resets,
            clk_ens,
            ctrls,
            regs ,
            irqs ,
            pages,
        }
    }
}

#[derive(Clone, Debug, Default)]
/// Define ports for a RIF interface
pub struct RifIntfPorts (Vec<PortInfo>);

impl RifIntfPorts {
    pub fn new(intf: &Interface) -> Self {
        let ports =
        match intf {
            Interface::Default => vec![
                PortInfo::new_intf(
                    "if_rif".to_owned(),
                    "rif_if".to_owned(),
                    "SW register interface".to_owned())
            ],
            Interface::Apb => vec![
                PortInfo::new("paddr".to_owned(), PortWidth::Address, PortDir::In, "APB Address".to_owned(), 0),
                PortInfo::new_in("psel".to_owned(), "APB Select".to_owned()),
                PortInfo::new_in("penable".to_owned(), "APB Enable".to_owned()),
                PortInfo::new_in("pwrite".to_owned(), "APB Write".to_owned()),
                PortInfo::new("pwdata".to_owned(), PortWidth::Data, PortDir::In, "APB Write Data".to_owned(), 0),
                PortInfo::new("prdata".to_owned(), PortWidth::Data, PortDir::Out, "APB Read Data".to_owned(), 0),
                PortInfo::new_out("pready".to_owned(), "APB Ready".to_owned()),
                PortInfo::new_out("pslverr".to_owned(), "APB Slave Error".to_owned()),
            ],
            Interface::Uaux => vec![
                PortInfo::new("uaux_addr".to_owned(), PortWidth::Address, PortDir::In, "AUX Address".to_owned(), 0),
                PortInfo::new_in("uaux_en".to_owned(), "AUX Enable".to_owned()),
                PortInfo::new_in("uaux_cmt_phase".to_owned(), "AUX Commit status".to_owned()),
                PortInfo::new_in("uaux_cmt_valid".to_owned(), "AUX Commit Valid".to_owned()),
                PortInfo::new_in("uaux_read".to_owned(), "AUX Read".to_owned()),
                PortInfo::new_in("uaux_write".to_owned(), "AUX Write".to_owned()),
                PortInfo::new("uaux_wdata".to_owned(), PortWidth::Data, PortDir::In, "AUX Write Data".to_owned(), 0),
                PortInfo::new("uaux_rdata".to_owned(), PortWidth::Data, PortDir::Out, "AUX Read Data".to_owned(), 0),
                PortInfo::new_out("uaux_busy".to_owned(), "AUX Busy".to_owned()),
                PortInfo::new_out("uaux_illegal".to_owned()  , "SR/LR illegal".to_owned()),
                PortInfo::new_out("uaux_k_rd".to_owned()     , "AUX read privilege violation".to_owned()),
                PortInfo::new_out("uaux_k_wr".to_owned()     , "AUX write privilege violation".to_owned()),
                PortInfo::new_out("uaux_unimpl".to_owned()   , "AUX unimplemented address".to_owned()),
                PortInfo::new_out("uaux_serial_sr".to_owned(), "AUX SR group flush ".to_owned()),
                PortInfo::new_out("uaux_strict_sr".to_owned(), "AUX SR single flush".to_owned()),
            ],
            Interface::Custom(name) => vec![
                PortInfo::new_intf(
                    format!("if_{}", name.strip_suffix("_if").unwrap_or(name)),
                    name.to_owned(),
                    "SW register interface".to_owned())
            ],
        };
        RifIntfPorts(ports)
    }

    pub fn iter(&self) -> impl Iterator<Item=&PortInfo> {
        self.0.iter()
    }
}