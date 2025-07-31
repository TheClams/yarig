use std::{collections::HashSet, ops::Deref};

use crate::{
    comp::{
        comp_inst::{Comp, CompInst, RifInst, RifmuxInst},
        hw_info::{PortDir, PortInfo, RifIntfPorts, SignalDecl, SignalDef, SignalInfo, SignalKind}
    },
    parser::parser_expr::ParamValues,
    rifgen::{
        order_dict::OrderDict, Access, CastInfo, ClkEn, ClockingInfo, EnumEntry, EnumKind, ExprId, ExternalKind, FieldHwKind, FieldSwKind, GenericRange, Interface, InterruptClr, InterruptRegKind, InterruptTrigger, LimitValue, LogicExpr, RegPulseKind, ResetDef, SignalRange}
};

use super::{
    casing::{Casing::{self, Snake}, ToCasing},
    gen_common::{CompInfo, GeneratorBase, RifList}
};

/// Trait to implement generator for hardware implementation (SV, VHDL, ...)
#[allow(unused_variables)]
pub trait GeneratorHw : GeneratorBase {

    /// Support SystemVerilog like interfaces
    const SUPPORT_INTF      : bool = true;
    /// Support implicit binding
    const SUPPORT_IMPL_BIND : bool = true;

    const IF_RIF_FIELDS: [&str; 11] = ["addr", "en", "rd_wrn", "wr_data", "rd_data", "done", "err_addr", "err_access", "done_next", "err_addr_next", "err_access_next"];

    /// Flag when register constants (address/reset) should be generated
    fn has_const_reg(&self) -> bool {false}

    /// Flag when field constants (mask, lsb, msb, reset) should be generated
    fn has_const_field(&self) -> bool {false}

    /// Write generic header for a file
    fn write_file_header(&mut self) {}

	/// Main generator function
    fn gen_all(&mut self, obj: &Comp) -> Result<(), Box<dyn std::error::Error>> {
        // Call relevant generator (Rif or Rifmux)
        match obj {
            Comp::Rif(rif) => {
                self.set_comp(rif.deref().into(), false);
                // self.set_comp(rif.deref().into(), false);
                self.gen_rif_pkg(rif)?;
                self.gen_rif(rif)?;
            }
            Comp::Rifmux(rifmux) => {
                self.gen_rifmux_pkg(rifmux)?;
                self.gen_rifmux(rifmux)?;
                // Generate include file
                if !self.setting().gen_inc.is_empty() {
                    let gen_all = self.setting().is_gen_all();
                    let rif_list = RifList::new(rifmux, true);
                    for (rif,_) in rif_list.iter() {
                        if !gen_all && !self.setting().is_gen_inc(rif) {
                            continue;
                        }
                        self.set_comp((*rif).into(), true);
                        self.gen_rif_pkg(rif)?;
                        self.gen_rif(rif)?;
                    }
                }
                // Generate Top
                if rifmux.top.is_some() {
                    self.gen_riftop(rifmux)?;
                }
            }
            // Nothing to do for external RIF
            Comp::External(_) => {},
        }
        Ok(())
    }

    /// Generate package containing enum, type and and structure definition
    fn gen_rif_pkg(&mut self, rif: &RifInst) -> Result<(), Box<dyn std::error::Error>> {
    	let rif_name = self.casing(&rif.name(true));
        self.write_file_header();
        self.write_rif_pkg_header(rif);
        // Constants definition
        self.write_rif_pkg_const(&rif.type_name, rif.addr_width, rif.data_width, &rif.params);
        // Enum definition
        let mut enums = rif.enum_defs.iter().filter(|e| e.is_local_type()).peekable();
        if enums.peek().is_some() {
            self.write_comment(1, "Enums");
            for enum_def in rif.enum_defs.iter().filter(|e| e.is_local_type()) {
                let width = (usize::BITS - (enum_def.len()-1).leading_zeros()) as u8;
                self.write_enum_header(&enum_def.name, width);
                let mut entries = enum_def.iter().peekable();
                while let Some(entry) = entries.next() {
                    self.write_enum_entry(entry, entries.peek().is_none());
                }
                self.write_enum_footer(&enum_def.name, width);
            }
        }
        // Structures
        // Create two structures (hardware/software) per register type
        let mut names : Vec<String> = Vec::new();
        for hw_reg in rif.reg_impl_defs.values().filter(|r| r.pkg.is_none()) {
            let mut hw_fields : Vec<SignalDecl> = Vec::new();
            let mut sw_fields : Vec<SignalDecl> = Vec::new();
            // Check if any fields is an array to control if structure should be packed or not
            names.clear();
            // Iterate over all register fields to add them in the structs
            for f in hw_reg.fields.iter() {
                // Create signal declaration
                let width = if f.sw_kind.is_password() {1} else {f.width};
                let name = f.name.to_casing(Snake);
                let Some(ctrl) = hw_reg.regs_ctrl.get(f.ctrl_idx) else {
                    return Err(format!("Field {}.{name} points to ctrl {} but max is {}",hw_reg.name, f.ctrl_idx, hw_reg.regs_ctrl.len()).into())
                };
                let kind : SignalKind = match &f.enum_kind {
                    EnumKind::Type(n) => SignalKind::Custom((None, n.to_owned())),
                    _ => if f.signed {SignalKind::Signed(width)} else {SignalKind::Unsigned(width)}
                };
                let field_name = if f.sw_kind.is_password() {format!("{name}_locked")} else {name.to_owned()};
                let field_decl = SignalDecl::new(
                    SignalDef::new_arr(field_name, kind, f.array.into()),
                    f.description.get_short(false)
                );
                // Add field to SW structure writable by firmware or readable by hardware
                if f.has_sw_value() && (!f.is_local() || ctrl.external.is_rw()) {
                    sw_fields.push(field_decl.clone());
                }
                // Add field to HW structure if written by hardware
                if f.has_hw_value() || ctrl.external.is_rw() {
                    hw_fields.push(field_decl);
                }
                // Add special fields
                for kind in f.hw_kind.iter() {
                    // Write modifiers: Write Enable, clr/set/tgl
                    if kind.has_write_mod() {
                        if let Some(d) = SignalDecl::from_hw_kind(kind, &hw_reg.name, &name) {
                            if !names.iter().rev().any(|n| n==d.name()) {
                                names.push(d.name().to_owned());
                                hw_fields.push(d);
                            }
                        }
                    }
                    // Counter need multiple extra fields
                    else if let FieldHwKind::Counter(info) = kind {
                        if info.clr {
                            hw_fields.push(SignalDecl::new_bit(
                                format!("{name}_clr"),
                                format!("Clear counter {name}")));
                        }
                        if info.is_up() {
                            hw_fields.push(SignalDecl::new_bit(
                                format!("{name}_incr_en"),
                                format!("Increment counter {name}")));
                        }
                        if info.is_down() {
                            hw_fields.push(SignalDecl::new_bit(
                                format!("{name}_decr_en"),
                                format!("Decrement counter {name}")));
                        }
                        if info.incr_val > 1 {
                            hw_fields.push(SignalDecl::new_bus(
                                format!("{name}_incr_val"), info.incr_val as u16, f.signed,
                                format!("Increment value for counter {name}")));
                        }
                        if info.decr_val > 1 {
                            hw_fields.push(SignalDecl::new_bus(
                                format!("{name}_decr_val"), info.decr_val as u16, f.signed,
                                format!("Decrement value for counter {name}")));
                        }
                        if info.event || info.sat {
                            sw_fields.push(SignalDecl::new_bit(
                                format!("{name}_event"),
                                format!("Pulse high when {name} wrap/saturate")));
                        }
                    }
                }
                if let FieldSwKind::Password(info) = &f.sw_kind {
                    if info.has_hold() {
                        sw_fields.push(SignalDecl::new_bit(
                            format!("{name}_hold"),
                            format!("High when {name}_locked is not changed by register access")));
                    }
                }
                // Clear
                if let Some(clr_expr) = &f.clear {
                    let kind = FieldHwKind::Clear(Some(clr_expr.to_owned()));
                    if let Some(d) = SignalDecl::from_hw_kind(&kind, &hw_reg.name, &name) {
                        if names.iter().rev().any(|n| n==d.name()) {
                            names.push(d.name().to_owned());
                            hw_fields.push(d);
                        }
                    }
                }
                // Lock signal from hardware
                if let Some(lock) = f.lock.local_field(&hw_reg.name) {
                    if !lock.is_empty() && !names.iter().rev().any(|n| n==lock) {
                        names.push(lock.to_owned());
                        hw_fields.push(SignalDecl::new_bit(
                            lock.to_owned(),
                            "High to lock some field write access".to_owned()));
                    }
                }
            }
            // Add fields for register pulse and external access
            let is_multi_pulse = hw_reg.is_multi_pulse();
            let is_multi_ext = hw_reg.is_multi_ext();
            for ctrl in hw_reg.regs_ctrl.iter() {
                let (sep,name) = if is_multi_pulse {("_",&*ctrl.name)} else {("","")};
                let desc = "Pulse high when register";
                for pulse in ctrl.pulse.iter() {
                    let field_decl = match pulse {
                        RegPulseKind::Write(_)  => SignalDecl::new_bit(format!("p_{name}{sep}write"), format!("{desc} {} is written", ctrl.name)),
                        RegPulseKind::Read(_)   => SignalDecl::new_bit(format!("p_{name}{sep}read"), format!("{desc} {} is read", ctrl.name)),
                        RegPulseKind::Access(_) => SignalDecl::new_bit(format!("p_{name}{sep}acc"), format!("{desc} {} is accessed", ctrl.name)),
                    };
                    sw_fields.push(field_decl);
                }
                if ctrl.external != ExternalKind::None {
                    let (sep,name) = if is_multi_ext {("_",&*ctrl.name)} else {("","")};
                    hw_fields.push(SignalDecl::new_bit(
                        format!("ext_{name}{sep}done"),
                        format!("Pulse high when read/write operation on register {} is complete", ctrl.name))
                    );
                    if matches!(ctrl.external, ExternalKind::ReadWrite | ExternalKind::Write) {
                        sw_fields.push(SignalDecl::new_bit(
                            format!("ext_{name}{sep}write"),
                            format!("Pulse high to start a write operation on register {}", ctrl.name))
                        );
                    }
                    if matches!(ctrl.external, ExternalKind::ReadWrite | ExternalKind::Read) {
                        sw_fields.push(SignalDecl::new_bit(
                            format!("ext_{name}{sep}read"),
                            format!("Pulse high to start a read operation on register {}", ctrl.name))
                        );
                    }
                }
            }
            // Write the software structure (if not empty)
            if !sw_fields.is_empty() {
                let type_name = format!("t_{}_sw",hw_reg.name.to_casing(Snake));
                self.write_struct_header(&type_name, &sw_fields);
                let mut fields = sw_fields.iter().peekable();
                while let Some(f) = fields.next() {
                    self.write_struct_field(f, fields.peek().is_none());
                }
                self.write_struct_footer(&type_name);
            }
            // Write the hardware structure (if not empty)
            if !hw_fields.is_empty() {
                let type_name = format!("t_{}_hw",hw_reg.name.to_casing(Snake));
                self.write_struct_header(&type_name, &hw_fields);
                let mut fields = hw_fields.iter().peekable();
                while let Some(f) = fields.next() {
                    self.write_struct_field(f, fields.peek().is_none());
                }
                self.write_struct_footer(&type_name);
            }
        }

        let basename_uc = rif.type_name.to_uppercase();
        // Create register constant if enabled
        if self.has_const_reg() {
            for reg in rif.iter_reg() {
                let regname = reg.name().to_uppercase();
                let decl = SignalDef::new_bus(format!("C_ADDR_{basename_uc}_{regname}"), rif.addr_width.into(), false);
                self.write_const(&decl.into(), LogicExpr::ValueU(reg.addr.into(), rif.addr_width.into()));
                let decl = SignalDef::new_bus(format!("C_RESET_{basename_uc}_{regname}"), rif.data_width.into(), false);
                self.write_const(&decl.into(), LogicExpr::ValueU(reg.reset, rif.data_width.into()));
            }
            self.write("\n");
        }

        if self.has_const_field() {
            for reg in rif.iter_reg() {
                let base = format!("{basename_uc}_{}", reg.name().to_uppercase());
                for f in reg.fields.iter() {
                    let fieldname = f.name_flat().to_uppercase();
                    let decl  = SignalDef::new_bus(format!("C_MSK_{base}_{fieldname}  "), rif.data_width.into(), false);
                    let value = LogicExpr::ValueU((1_u128<<f.width).wrapping_sub(1), rif.data_width.into());
                    self.write_const(&decl.into(), value);
                    let decl  = SignalDef::new_int(format!("C_IL_{base}_{fieldname}   "));
                    let value = LogicExpr::ValueU(f.lsb.into(), 8);
                    self.write_const(&decl.into(), value);
                    let decl  = SignalDef::new_int(format!("C_IH_{base}_{fieldname}   "));
                    let value = LogicExpr::ValueU(f.msb().into(), 8);
                    self.write_const(&decl.into(), value);
                    let decl  = SignalDef::new_bus(format!("C_RESET_{base}_{fieldname}"), f.width.into(), f.is_signed());
                    let value = if f.is_signed() {
                        LogicExpr::ValueI(f.reset(None) as i128, f.width.into())
                    } else {
                        LogicExpr::ValueU(f.reset(None), f.width.into())
                    };
                    self.write_const(&decl.into(), value);
                }
            }
            self.write("\n");
        }

        // Add end of package and save file
        self.write_rif_pkg_footer(rif);
        self.save(&self.filename_rif_pkg(rif))?;
        Ok(())
    }

    // Hooks for RIF package
    fn write_rif_pkg_header(&mut self, rif: &RifInst) {
        self.write_pkg_header(&rif.type_name);
    }
    fn write_rif_pkg_footer(&mut self, rif: &RifInst) {
        self.write_pkg_footer(&rif.type_name);
    }

    fn write_rif_pkg_const(&mut self, basename: &str, addr_width: u8, data_width: u8, params: &ParamValues) {
        let basename_uc = basename.to_uppercase();
        let decl: SignalDecl = SignalDef::new_int(format!("C_{basename_uc}_ADDR_W")).into();
        self.write_const(&decl, LogicExpr::ValueU(addr_width.into(), 8));
        let decl: SignalDecl = SignalDef::new_int(format!("C_{basename_uc}_DATA_W")).into();
        self.write_const(&decl, LogicExpr::ValueU(data_width.into(), 8));
        for (k, &v) in params.items() {
            let name = format!("C_{basename_uc}_{}", k.to_uppercase());
            let decl: SignalDecl = if v==0 || v==1 {
                SignalDef::new_bit(name).into()
            } else {
                SignalDef::new_int(name).into()
            };
            self.write_const(&decl, LogicExpr::ValueI(v as i128, 32));
        }
        self.write("\n");
    }

    /// Generate RIF module
    fn gen_rif(&mut self, rif: &RifInst) -> Result<(), Box<dyn std::error::Error>> {
        self.set_rif_info(rif);
        let rif_name = self.casing(&rif.name(false));
        let rif_pkg_name = self.casing(&rif.name(true));
        let hw_clk = rif.hw_clocking.first().unwrap_or(&rif.sw_clocking);
        let addr_shift = (rif.data_width as f32).log2().ceil() as u16 - 3; // Min data width is 8 bits
        self.write_file_header();
        self.write_module_decl_header(&rif_name);
        if !rif.generics.is_empty() {
            self.write_module_generic_header();
            let mut generics = rif.generics.items().peekable();
            while let Some((name,range)) = generics.next() {
                self.write_module_generic_decl(name, range, generics.peek().is_none());
            }
        }
        self.write_module_port_header();

        // Clocks/Reset/Clear
        let mut list_clocking = HashSet::with_capacity(2);
        self.add_clocking_port(&rif.sw_clocking, &mut list_clocking, false);
        for hw_clk in rif.hw_clocking.iter() {
            self.add_clocking_port(hw_clk, &mut list_clocking, true);
        }
        // Clock enables
        for clk_en in rif.ports.clk_ens.iter() {
            self.write_port_decl(clk_en, None, false);
        }
        // Control signals
        if !rif.ports.ctrls.is_empty() {
            self.write_comment(1, "Input signals");
            for ctrl in rif.ports.ctrls.iter() {
                self.write_port_decl(ctrl, None, false);
            }
        }
        // Inputs
        let mut ports_out = Vec::new(); // Collect output ports
        let mut interrupts = Vec::new(); // Collect interrupt registers to create the corresponding IRQ line
        self.write_comment(1, "Input registers");
        for (group_name, hw_reg) in rif.hw_regs.items() {
            let hw_reg_def = rif.get_hw_reg(&hw_reg.group);
            let pkg_name = format!("{}_pkg", if let Some(pkg) = &hw_reg_def.pkg {pkg} else {&rif_pkg_name});
            let kind = if hw_reg.intr_derived {"sw"} else {"hw"};
            let group_name = self.casing(group_name);
            let prefix = if hw_reg.port.is_in() {""} else {"rif_"};
            let mut port = PortInfo::new(
                group_name.clone(),
                SignalKind::Custom((
                    Some(self.casing(&pkg_name)),
                    format!("t_{}_{kind}", self.casing(&hw_reg.group)))),
                PortDir::In,
                hw_reg.dim.clone(),
                hw_reg_def.description.get_short(false)
            );
            if hw_reg.port.is_in() {
                self.write_port_decl(&port, None, false);
            }
            if hw_reg.port.is_out() {
                port.def.name = format!("rif_{}", port.name());
                port.def.kind = SignalKind::Custom((
                    Some(self.casing(&pkg_name)),
                    format!("t_{}_sw", self.casing(&hw_reg.group))));
                port.dir = PortDir::Out;
                ports_out.push(port);
            }
            if !hw_reg_def.interrupt.is_empty() && !hw_reg.intr_derived {
                interrupts.push(group_name.clone());
                for info in hw_reg_def.interrupt.iter().skip(1) {
                    interrupts.push(format!("{}_{}", group_name, info.name));
                }
            }
        }

        // Outputs
        self.write_comment(1, "Output registers");
        for port in ports_out.iter() {
            self.write_port_decl(port, None, false);
        }

        // Interrupt lines
        for name in interrupts {
            let port_irq = PortInfo::new_out(
                format!("rif_{name}_irq"),
                format!("High when one interrupt field of {name} is asserted"));
            self.write_port_decl(&port_irq, None, false);
        }

        // Add interface to external pages and collect them for later
        if rif.pages.iter().any(|p| p.is_external()) {
            self.write_comment(1, "External pages interface");
        }
        let mut ext_pages = Vec::new();
        for page in rif.pages.iter() {
            if let Some(width) = &page.external {
                let name = self.casing(&format!("if_page_{}", page.name));
                ext_pages.push((name.clone(), page.addr, *width));
                let port = PortInfo::new_intf(
                    name.clone(),
                    "rif_if".to_owned(), "ctrl".to_owned(),
                    format!("Interface to page {}", page.name.to_casing(Casing::Title)));
                self.write_port_decl(&port, None, false);
                continue;
            }
        }
        // Create a few expr used many times
        let rif_en : LogicExpr = ("if_rif","en").into();
        let rd_wrn : LogicExpr = ("if_rif","rd_wrn").into();

        // Add main control interface
        if !Self::SUPPORT_INTF {
            self.write_comment(1, "Register SW interface");
        }
        self.write_intf_ports(&rif.interface);
        self.write_module_decl_footer(&rif_name);

        // Signals declaration
        let w = rif.data_width as u16;
        self.write_comment_box("Signals declaration");
        self.write_signal_decl(&("rif_addr_l", rif.addr_width as u16 - addr_shift).into());
        self.write_signal_decl(&("rif_read_data_l", w).into());
        self.write_signal_decl(&"rif_err_addr_l".into());
        self.write_signal_decl(&"rif_err_access_l".into());
        self.write_signal_decl(&"rif_done_next".into());
        self.write("\n");

        // Declare Decode pulse / readback value per register
        for page in rif.pages.iter().filter(|p| p.external.is_none()) {
            for reg in page.regs.iter() {
                let name = reg.name().to_casing(Snake);
                if reg.has_decode() {
                    self.write_signal_decl(&format!("{name}__decode").as_str().into());
                }
                self.write_signal_decl(&(format!("{name}__read_data").as_str(), w).into());
            }
        }
        self.write("\n");

        // Declare local signal per register group
        for (inst_name, hw_reg) in rif.hw_regs.items().filter(|(_,r)| !r.intr_derived) {
            let group_name = self.casing(inst_name);
            let hw_reg_def = rif.get_hw_reg(&hw_reg.group);
            let reg_dim = hw_reg.dim.val();
            let group_type_sw = format!("t_{}_sw", hw_reg.group);
            let group_type_hw = format!("t_{}_hw", hw_reg.group);
            let pkg_base = if let Some(pkg) = &hw_reg_def.pkg {pkg} else {&rif_pkg_name};
            let pkg_name = format!("{pkg_base}_pkg");
            // Local register
            if hw_reg_def.is_local() {
                self.write_signal_decl(&SignalDef::new_ud_arr(
                    format!("rif_{group_name}"),
                    pkg_name.clone(), group_type_sw.clone(), hw_reg.dim.clone()).into());
            }
            // Add signal to handle out-of-limit check
            for limit in hw_reg.limits.iter() {
                self.write_signal_decl(&SignalDef::new_bit(format!("{limit}__check")).into());
            }
            for idx_u16 in 0..reg_dim.max(1) {
                let idx = if reg_dim > 0 {format!("{idx_u16}")} else {"".to_owned()};
                // Interrupt register
                if hw_reg_def.is_interrupt() {
                    // Declare all interrupt related signal: event, en, pending, mask, alts
                    for intr_info in hw_reg_def.interrupt.iter() {
                        let name = if intr_info.name.is_empty() {
                            group_name.to_owned()
                        } else {
                            format!("{}_{}", group_name, intr_info.name)
                        };
                        // Declare the sw struct if not an output
                        if !hw_reg.port.is_out() {
                            self.write_signal_decl(&SignalDef::new_ud(
                                format!("rif_{name}{idx}"),
                                pkg_name.clone(), group_type_sw.clone()).into());
                        }
                        // Interrupt need a local signal (and between input and enable)
                        self.write_signal_decl(&SignalDef::new_ud(
                            format!("{name}{idx}_l"),
                            pkg_name.clone(), group_type_hw.clone()).into());
                        // Add delay register if trigger works on edges
                        if intr_info.edge_trigger() {
                            self.write_signal_decl(&SignalDef::new_ud(
                                format!("{name}{idx}_d1"),
                                pkg_name.clone(), group_type_hw.clone()).into());
                        }
                        // Add optional enable/mask register
                        if intr_info.enable.is_some() {
                            let n = format!("{inst_name}_en");
                            if let Some(hw_reg_en) = rif.hw_regs.get(&n) {
                                if !hw_reg_en.port.is_out() {
                                    self.write_signal_decl(&SignalDef::new_ud(
                                        format!("rif_{name}{idx}_en"),
                                        pkg_name.clone(), group_type_hw.clone()).into());
                                }
                            }
                        }
                        if intr_info.mask.is_some() {
                            let n = format!("{inst_name}_mask");
                            if let Some(hw_reg_mask) = rif.hw_regs.get(&n) {
                                if !hw_reg_mask.port.is_out() {
                                    self.write_signal_decl(&SignalDef::new_ud(
                                        format!("rif_{name}{idx}_mask"),
                                        pkg_name.clone(), group_type_hw.clone()).into());
                                }
                            }
                        }
                        // Internal pending signal is always present (used to generate the irq output)
                        self.write_signal_decl(&SignalDef::new_ud(
                            format!("rif_{name}{idx}_pending"),
                            pkg_name.clone(), group_type_hw.clone()).into());
                        self.write_signal_decl(&SignalDef::new_bit(format!("clk_en_intr_{name}{idx}")).into());
                        // Add next signal for each field
                        for f in hw_reg_def.fields.iter() {
                            let f_name = self.casing(&f.name);
                            let w = f.width + (if f.is_counter() {1} else {0});
                            self.write_signal_decl(&SignalDef::new_bus(
                                format!("{name}{idx}_{f_name}__cleared"), f.width, false).into());
                            self.write_signal_decl(&SignalDef::new_bus(
                                format!("{name}{idx}_{f_name}__next"), f.width, false).into());
                            if intr_info.enable.is_some() {
                                self.write_signal_decl(&SignalDef::new_bus(
                                    format!("{name}{idx}_en_{f_name}__next"), f.width, false).into());
                            }
                            if intr_info.mask.is_some() {
                                self.write_signal_decl(&SignalDef::new_bus(
                                    format!("{name}{idx}_mask_{f_name}__next"), f.width, false).into());
                            }
                        }

                    }
                    continue;
                }
                // Field combinatorial next value
                for f in hw_reg_def.fields.iter() {
                    let f_name = format!("{group_name}{idx}_{}", self.casing(&f.name));
                    // Skip external field
                    let Some(ctrl) = hw_reg_def.regs_ctrl.get(f.ctrl_idx) else {
                        return Err(format!("Field {}.{} points to ctrl {} but max is {}",
                            hw_reg_def.name, f.name, f.ctrl_idx, hw_reg_def.regs_ctrl.len()).into())
                    };
                    if ctrl.is_external() {
                        continue;
                    }
                    // No next for combinatorial pulse or read only field from hardware with no register
                    if f.sw_kind.is_pulse_comb() || (f.sw_kind==FieldSwKind::ReadOnly && !f.has_write_mod() && !f.is_counter()) {
                        continue;
                    }
                    let mut sig_kind = SignalKind::from_field(f, pkg_base, &group_name);
                    if f.is_counter() {
                        sig_kind.set_width(f.width+1);
                    }
                    if f.array > 0 {
                        for i in 0..f.array {
                            self.write_signal_decl(&SignalDef::new(
                                format!("{f_name}{i}__next"), sig_kind.clone()).into());
                        }
                    } else {
                        self.write_signal_decl(&SignalDef::new(
                            format!("{f_name}__next"), sig_kind.clone()).into());
                    }
                    // Add register to store local value when register is not visible at the output
                    if f.is_local() {
                        self.write_signal_decl(&SignalDef::new(
                            format!("{f_name}__reg"), sig_kind.clone()).into());
                    }
                }
            }
        }

        let comp_info : CompInfo = rif.into();

        // Create local rif interface if not default
        if !rif.interface.is_default() {
            self.write_rif_decl(&comp_info, true, &rif.sw_clocking.clk, &rif.sw_clocking.rst.name);
        }

        self.write_signal_decl_footer(true);

        // Add interface bridge to the internal rif_if
        // Nothing is done if already using rif_if
        self.write_intf_bridge(&rif.interface, &comp_info, &rif.sw_clocking.clk, &rif.sw_clocking.rst.name);

        // Connect interface to internal logic
        self.write_comment_box("Interface handling");
        let signals: Vec<SignalInfo> = vec![
            // Mask err with RIF enable signal
            SignalInfo::new(("if_rif","err_addr").into()  , LogicExpr::ValueU(0, 1),
                LogicExpr::and("rif_err_addr_l".into(), rif_en.clone())),
            SignalInfo::new(("if_rif","err_access").into(), LogicExpr::ValueU(0, 1),
                LogicExpr::and("rif_err_access_l".into(), rif_en.clone())),
            // Done is a direct copy
            SignalInfo::new(("if_rif","done").into(), LogicExpr::ValueU(0, 1), "rif_done_next".into()),
            // Read data updated only on read access
            SignalInfo::new_with_en(("if_rif","rd_data").into(), LogicExpr::ValueU(0, rif.data_width.into()),
                "rif_read_data_l".into(),
                Some(LogicExpr::and("rif_done_next".into(), rd_wrn.clone()))),
        ];
        if self.nb_pipe() == 0 {
            for s in signals {
                self.write_assign(s.name, s.value);
            }
        } else {
            self.write_process_seq(
                &rif.sw_clocking.clk,
                &rif.sw_clocking.rst,
                "proc_if_rif",
                &signals,
            );
        }

        self.write_assign(("if_rif","done_next      ").into(), "rif_done_next   ".into());
        self.write_assign(("if_rif","err_addr_next  ").into(), "rif_err_addr_l  ".into());
        self.write_assign(("if_rif","err_access_next").into(), "rif_err_access_l".into());
        self.write("\n");
        let addr_bus = ExprId::new_field_range("if_rif".to_owned(), None, "addr".to_owned(), Some(SignalRange::new(addr_shift as u8, rif.addr_width-1)));
        // format!("if_rif.addr[{}:{}]", rif.addr_width-1, addr_shift);
        self.write_assign("rif_addr_l".into(), addr_bus.into());

        // Decode process
        self.write_process_comb_header("decode");
        self.write_assign_comb(2, "rif_read_data_l".into(), LogicExpr::ValueU(0, rif.data_width.into()));
        if ext_pages.is_empty() {
            self.write_assign_comb(2, "rif_done_next".into(), rif_en.clone());
            self.write_assign_comb(2, "rif_err_addr_l".into(), LogicExpr::ValueU(1, 1));
        } else {
            let page_en = LogicExpr::Or(
                ext_pages.iter().map(|(n,_,_)| LogicExpr::from((n.as_str(),"en"))).collect());
            let mut done_next : Vec<LogicExpr> = Vec::new();
            done_next.push(LogicExpr::and(rif_en.clone(), LogicExpr::not(page_en.clone())));
            done_next.extend(
                ext_pages.iter().map(|(n,_,_)|
                    LogicExpr::and((n.as_str(),"en").into(),(n.as_str(),"done").into())
                )
            );
            self.write_assign_comb(2, "rif_done_next".into(), LogicExpr::Or(done_next));
            self.write_assign_comb(2, "rif_err_addr_l".into(), LogicExpr::not(page_en));
        }
        self.write_assign_comb(2, "rif_err_access_l".into(), LogicExpr::ValueU(1, 1));
        // Set the decode signal default value
        for page in rif.pages.iter().filter(|p| p.external.is_none()) {
            for reg in page.regs.iter().filter(|r| r.has_decode()) {
                let name = self.casing(&format!("{}__decode", reg.name()));
                self.write_assign_comb(2, name.into(), LogicExpr::ValueU(0, 1));
            }
        }
        // Decode address
        self.write_match_header("rif_addr_l");
        let addr_l_w = rif.addr_width as usize - addr_shift as usize ;
        for page in rif.pages.iter().filter(|p| p.external.is_none()) {
            for reg in page.regs.iter() {
                let name_flat = self.casing(&reg.name());
                let group_name = self.casing(&reg.group_name());
                let addr = (reg.addr + page.addr) as u128 >> addr_shift;
                self.write_match_case_header(LogicExpr::ValueU(addr, addr_l_w));
                // Set the decode signal high when matching address
                // and the register contains no field with limit or all limit check pass
                let field_limit: Vec<(String, String)> = reg
                    .fields
                    .iter()
                    .filter(|field| field.limit.value != LimitValue::None)
                    .map(|field| (field.name.to_owned(), field.limit.bypass.to_owned()))
                    .collect();
                if reg.has_decode() {
                    let name = self.casing(&format!("{}__decode", reg.name()));
                    let value =
                        if field_limit.is_empty() {
                            reg.optional.as_ref().unwrap_or(&LogicExpr::ValueU(1, 1)).to_owned()
                        } else {
                            let checks : Vec<LogicExpr> = field_limit.iter().map(|(n,bypass)| {
                                let idx = reg.array.idx_str(false);
                                let check : LogicExpr = format!("{group_name}{idx}_{n}__check").into();
                                if bypass.is_empty() {check}
                                else {LogicExpr::or(check, bypass.as_str().into())}
                            }).collect();
                            let expr = LogicExpr::or(rd_wrn.clone(), LogicExpr::And(checks));
                            if let Some(opt) = &reg.optional {
                                LogicExpr::and(opt.clone(), expr)
                            } else {
                                expr
                            }

                        };
                    self.write_assign_comb(4, name.into(), value);
                }
                // Copy corresponding read_data signal
                self.write_assign_comb(4, "rif_read_data_l ".into(), format!("{name_flat}__read_data").into());
                // Address is valid if register is not optional
                let err_addr = if let Some(opt) = &reg.optional {
                    LogicExpr::ite(opt.to_owned(), LogicExpr::ValueU(0, 1), LogicExpr::ValueU(1, 1))
                } else {
                    LogicExpr::ValueU(0, 1)
                };
                self.write_assign_comb(4, "rif_err_addr_l  ".into(), err_addr);
                // Access error when writing a read-only field, reading a write only field,
                //  or writing one field outside its set value (when limits are defined)
                let err_val : LogicExpr = match reg.sw_access {
                    Access::RO => LogicExpr::not(rd_wrn.clone()),
                    Access::WO => rd_wrn.clone(),
                    Access::NA => LogicExpr::ValueU(1, 1),
                    Access::RW =>
                        if field_limit.is_empty() {
                            LogicExpr::ValueU(0, 1)
                        } else {
                            LogicExpr::not(format!("{name_flat}__decode").into())
                        }
                };
                self.write_assign_comb(4, "rif_err_access_l".into(), err_val);
                // Override done_next for external registers
                if reg.external!=ExternalKind::None {
                    let hw_reg_def = rif.get_hw_reg(&reg.group_type);
                    let idx = reg.array.idx_str(true);
                    let qual = if hw_reg_def.is_multi_pulse() {format!("_{name_flat}")} else {"".to_owned()};
                    let next : ExprId = (format!("{group_name}{idx}"), format!("ext{qual}_done")).into();
                    self.write_assign_comb(4, "rif_done_next".into(), next.into());
                }
                self.write_match_case_footer();
            }
        }
        // Default case with optional
        self.write_match_case_header("default".into());
        if !ext_pages.is_empty() {
            for (i,pn) in ext_pages.iter().map(|p| p.0.as_str()).enumerate() {
                let cond = LogicExpr::eq((pn,"done").into(), LogicExpr::ValueU(1, 1));
                if i==0 {
                    self.write_cond_if(4, cond);
                } else {
                    self.write_cond_else(4, Some(cond));
                }
                self.write_assign_comb(5, "rif_read_data_l ".into(), (pn,"rd_data").into());
                self.write_assign_comb(5, "rif_err_addr_l  ".into(), (pn,"err_addr").into());
                self.write_assign_comb(5, "rif_err_access_l".into(), (pn,"err_access").into());
            }
            self.write_cond_end(4);
        }
        self.write_match_case_footer();
        // Close match and process
        self.write_match_footer();
        self.write_process_comb_footer("decode");

        // Control the external page interface
        for (name, addr, width) in ext_pages.iter() {
            let addr_bus_lsb = ExprId::new_field_range("if_rif".to_owned(), None, "addr".to_owned(), Some(SignalRange::new(0, *width - 1)));
            let addr_bus_msb = ExprId::new_field_range("if_rif".to_owned(), None, "addr".to_owned(), Some(SignalRange::new(*width, rif.addr_width-1)));
            self.write_assign((name.as_str(),"addr   ").into(),addr_bus_lsb.into());
            self.write_assign((name.as_str(),"rd_wrn ").into(), rd_wrn.clone());
            self.write_assign((name.as_str(),"wr_data").into(),("if_rif","wr_data").into());
            // format!("if_rif.addr[{}:{}]", rif.addr_width-1, width);
            let addr_val = LogicExpr::ValueU((addr >> width) as u128, (rif.addr_width - width)  as usize);
            let addr_check = LogicExpr::eq(addr_bus_msb.into(), addr_val);
            let en_expr = LogicExpr::and(rif_en.clone(), addr_check);
            self.write_assign((name.as_str(),"en     ").into(), en_expr);
        }

        // Register process
        self.write_comment_box("Registers");
        let mut group_done : HashSet<String> = HashSet::with_capacity(rif.hw_regs.len());
        for page in rif.pages.iter().filter(|p| p.external.is_none()) {
            for reg in page.regs.iter() {
                let reg_impl = rif.get_hw_reg(&reg.group_type);
                // Save a few string to be reused
                let reg_name  = reg.name().to_casing(Snake); // Register Name with index append after
                let group_name = reg.group_name().to_casing(Snake); // Group name without index
                let reg_name_i   = reg.name_i().to_casing(Snake); // Register name with optional index in bracket
                let reg_idx    = if reg.array.dim_inst() > 0 {Some(reg.array.idx())} else {None};
                let group_id : ExprId = (group_name.clone(), reg_idx).into();
                let sw_groupd_id : ExprId = (format!("rif_{group_name}"), reg_idx).into();
                let reg_idxf   = if let Some(idx) = reg_idx {format!("{idx}")} else {"".to_owned()};
                let intr_suffix = reg.intr_info.0.get_suffix();
                let decode : LogicExpr = format!("{reg_name}__decode").into();
                let reg_def_idx = reg.def_idx();
                self.write("\n");
                self.write_comment(1, &format!("Register {reg_name_i}"));
                // Assign field
                if let Some(opt) = &reg.optional {
                    self.write_generate_if(opt.to_owned(), format!("gen_reg_{reg_name}"));
                }
                for field in reg.fields.iter() {
                    let field_impl = reg_impl.get_field(&field.name)?;
                    let partial  = field.to_range(reg_def_idx,false);
                    let field_range = field.to_range(reg_def_idx,true);
                    let field_name = self.casing(&field.name);
                    let field_name_flat = self.casing(&field.name_flat());
                    let reg_field_name = format!("{group_name}{intr_suffix}{reg_idxf}_{field_name_flat}");
                    let field_id = if field_impl.is_local() && field.has_write_mod() && !reg.is_external() {
                        ExprId::new_range(format!("{reg_field_name}__reg"), partial.clone())
                    } else {
                        ExprId::new_field_range(
                            format!("rif_{group_name}{intr_suffix}"), reg_idx,
                            field_name.clone(), field_range.clone())
                    };
                    let field_cast = field_impl.hdl_cast(&rif_pkg_name, &reg.reg_type);
                    let field_next_id = ExprId::new_range(format!("{reg_field_name}__next"), partial.clone());
                    let field_reset = LogicExpr::reset(&field.reset, field.width);
                    let reset_expr = if field_cast.is_custom() {
                        LogicExpr::Cast(field_cast.clone(), Box::new(field_reset))
                    } else {
                        field_reset
                    };
                    // Constant or Disabled field ? simply assign to its reset value
                    if field_impl.is_constant() || (field.is_disabled() && field.is_sw_write())  {
                        self.write_assign(field_id, reset_expr);
                        continue;
                    }

                    // Construct the field value from the bus with bit selection
                    // For non partial field, add proper casting (signed/enum)
                    let bus_range = if field.width > 1 {
                        SignalRange::new(field.lsb, field.msb())
                    } else {
                        SignalRange::new_bit(field.lsb)
                    };

                    let wr_data_raw = LogicExpr::Id(ExprId::new_field_range("if_rif".to_owned(), None, "wr_data".to_owned(), Some(bus_range)));
                    let wr_data_l = if field.is_signed() {LogicExpr::Cast(CastInfo::Signed, Box::new(wr_data_raw.clone()))} else {wr_data_raw.clone()};
                    let wr_data = LogicExpr::Cast(
                        if field_impl.is_partial {CastInfo::None} else {field_cast},
                        Box::new(wr_data_raw));

                    // Add logic for field with limit
                    if field.has_limit() {
                        let check_sig : ExprId = format!("{reg_field_name}__check").into();
                        let check_expr = match &field.limit.value {
                            LimitValue::Min(v) => LogicExpr::gte(wr_data_l.clone(), LogicExpr::reset(v, field.width)),
                            LimitValue::Max(v) => LogicExpr::lte(wr_data_l.clone(), LogicExpr::reset(v, field.width)),
                            LimitValue::MinMax(min, max) => {
                                let min_expr = LogicExpr::gte(wr_data_l.clone(), LogicExpr::reset(min, field.width));
                                let max_expr = LogicExpr::lte(wr_data_l.clone(), LogicExpr::reset(max, field.width));
                                LogicExpr::and(min_expr, max_expr)
                            },
                            LimitValue::List(l) => {
                                let v : Vec<LogicExpr> = l.iter()
                                    .map(|e| LogicExpr::eq(wr_data_l.clone(), LogicExpr::reset(e, field.width)))
                                    .collect();
                                LogicExpr::Or(v)
                            }
                            LimitValue::Enum => {
                                let Some(enum_name) = field.enum_kind.name() else {
                                    return Err(format!("Using `limit enum` on non-enum field {reg_field_name}!").into());
                                };
                                let enum_type = if let Some(pkg) = &reg_impl.pkg {
                                    if enum_name.contains(':') {enum_name.to_owned()}
                                    else {format!("{pkg}_pkg::{enum_name}")}
                                } else {
                                    enum_name.to_owned()
                                };
                                let enum_def = rif.get_enum_def(&enum_type)?;
                                let v : Vec<LogicExpr> = enum_def.iter()
                                    .map(|e| LogicExpr::eq(wr_data_l.clone(), LogicExpr::value(e.value as u128, field)))
                                    .collect();
                                LogicExpr::Or(v)
                            }
                            LimitValue::None => unreachable!(),
                        };
                        self.write_assign(check_sig, check_expr);
                    }

                    // For external register combinatorial assign from the interface bus
                    if reg.is_external() && field.is_sw_write() {
                        self.write_assign(field_id, wr_data);
                        continue;
                    }

                    // Combinatorial pulse : direct assign
                    if field.sw_kind.is_pulse_comb() {
                        let mut cond = LogicExpr::and(decode.clone(), rif_en.clone());
                        cond.push(LogicExpr::not(rd_wrn.clone()));
                        let val = LogicExpr::ite(cond, wr_data, LogicExpr::value(0, field));
                        self.write_assign(field_id, val);
                        continue;
                    }

                    // Counter event
                    if let Some(info) = field.counter_info() {
                        if info.sat || info.event {
                            let name = field_id.with_fsuffix("_event");
                            let next_msb = ExprId::new_idx(format!("{reg_field_name}__next"), (field.width-1) as u16);
                            let event = if field.is_signed() {
                                let ovfl = next_msb.with_idx(field.width as u16);
                                LogicExpr::neq(ovfl.into(), next_msb.clone().into())
                            } else {
                                let curr_msb : LogicExpr = field_id.with_range(SignalRange::new_bit(field.width-1)).into();

                                let incr = if info.incr_val == 0 {None} else {
                                    Some(LogicExpr::And([
                                        LogicExpr::not(next_msb.clone().into()),
                                        curr_msb.clone(),
                                        field_id.with_fsuffix("_incr_en").into()
                                    ].to_vec()))
                                };
                                let decr = if info.decr_val == 0 {None} else {
                                    Some(LogicExpr::And([
                                        next_msb.clone().into(),
                                        LogicExpr::not(curr_msb),
                                        field_id.with_fsuffix("_decr_en").into()
                                    ].to_vec()))
                                };
                                match (incr,decr) {
                                    (None   ,Some(d)) => d,
                                    (Some(i),None   ) => i,
                                    (Some(i),Some(d)) => LogicExpr::or(i,d),
                                    _ => unreachable!("A counter is either up or down"),
                                }
                            };
                            // When counter is writable by software, mask event when the access occurs
                            let rhs = if field.is_sw_write() {
                                let acc_kind = if field.sw_kind==FieldSwKind::ReadClr {
                                    LogicExpr::not(rd_wrn.clone())
                                } else {
                                    rd_wrn.clone()
                                };
                                let acc_bar = LogicExpr::Or([
                                    LogicExpr::not(decode.clone()),
                                    LogicExpr::not(rif_en.clone()),
                                    rd_wrn.clone()].to_vec());
                                LogicExpr::and(acc_bar,event)
                            } else {
                                event
                            };
                            self.write_assign(name, rhs);
                        }
                    }

                    // Generate intermediate signal for interrupt
                    if reg.is_intr() {
                        let intr_info = reg_impl.intr_info(reg)?;
                        let group_name_base = reg.group_name.to_casing(Snake);
                        // Local signal where interrupt vector is and with the optional enable signals
                        let intr_l = ExprId::new_field_range(format!("{group_name}_l"), None, field_name.to_owned(), field_range);

                        let mut rhs : LogicExpr = (ExprId::from((group_name_base, field_name.clone()))).into();
                        if intr_info.enable.is_some() {
                            let en : ExprId = (format!("rif_{group_name}_en"), field_name.clone()).into();
                            rhs = LogicExpr::and_b(rhs.clone(), en.into());
                        }
                        self.write_assign(intr_l.clone(), rhs);
                        // Next interrupt state
                        let intr_l = LogicExpr::Id(intr_l); // Convert ExprId into LogicExpr
                        let intr_d1 : LogicExpr = (ExprId::from((format!("{group_name}_d1"), field_name.clone()))).into();
                        let intr_set : LogicExpr = match field.intr_trig(intr_info.trigger) {
                            InterruptTrigger::High    => intr_l,
                            InterruptTrigger::Low     => LogicExpr::not_b(intr_l),
                            InterruptTrigger::Rising  => LogicExpr::and_b(intr_l, LogicExpr::not_b(intr_d1)),
                            InterruptTrigger::Falling => LogicExpr::and_b(LogicExpr::not_b(intr_l), intr_d1),
                            InterruptTrigger::Edge    => LogicExpr::xor(intr_l, intr_d1),
                        };
                        let mut clr_acc = LogicExpr::and(format!("{reg_name}__decode").into(), rif_en.clone());
                        let clr_val = match intr_info.clear {
                            InterruptClr::Read   => {
                                clr_acc.push(rd_wrn.clone());
                                LogicExpr::ValueU(0, field.width.into())
                            }
                            InterruptClr::Write0 => {
                                clr_acc.push(LogicExpr::not(rd_wrn.clone()));
                                LogicExpr::and_b(wr_data, field_id.clone().into())
                            }
                            InterruptClr::Write1 => {
                                clr_acc.push(LogicExpr::not(rd_wrn.clone()));
                                LogicExpr::and_b(LogicExpr::not_b(wr_data), field_id.clone().into())
                            }
                            InterruptClr::Hw => todo!("Hardware clear interrupt are not supported yet ! :(")
                        };
                        let intr_clr = LogicExpr::ite(clr_acc, clr_val, field_id.into());
                        let field_clr_id = ExprId::new_range(format!("{reg_field_name}__cleared"), partial.clone());
                        self.write_assign(field_clr_id.clone(), intr_clr);
                        let rhs = LogicExpr::or_b(intr_set, field_clr_id.into());
                        self.write_assign(field_next_id, rhs);
                        continue;
                    }

                    // Register derived from interrupt (i.e. enable/mask)
                    if reg.is_intr_derived() && reg.intr_info.0 !=InterruptRegKind::Pending {
                        let acc = LogicExpr::And([
                            decode.clone(),
                            rif_en.clone(),
                            LogicExpr::not(rd_wrn.clone())].to_vec());
                        let rhs = LogicExpr::ite(acc, wr_data, field_id.into());
                        self.write_assign(field_next_id, rhs);
                        continue;
                    }

                    // Path to hardware field
                    let field_sig : LogicExpr = group_id.with_path(field_name.clone(), field_range.clone()).into();

                    // Generate next value
                    if field.is_hw_write() || field.is_sw_write() {
                        let field_clken = if let ClkEn::Signal(clk_en) = &field_impl.clk_en {
                            clk_en.clone()
                        } else if let ClkEn::Signal(clk_en) = &reg_impl.clk_en {
                            clk_en.clone()
                        } else {
                            hw_clk.en.clone()
                        };
                        let mut if_then : Vec<(LogicExpr,LogicExpr)> = Vec::new();

                        // Handle registered pulse: need to maintain pulse high until clock enable is seen
                        if field.sw_kind.is_pulse() && !field_clken.is_empty() {
                            if_then.push((
                                LogicExpr::and(field_id.clone().into(), field_clken.clone().into()),
                                LogicExpr::ValueU(0, field.width.into())
                            ));
                        }
                        // Handle hardware access
                        if field.is_hw_write() {
                            let suffix_idx = field.partial_suffix();
                            for kind in field.hw_kind.iter() {
                                let path = kind.get_signal();
                                let ext = kind.get_suffix();
                                let ctrl_id = path_to_signal(path, ext, (&reg.group_type, &group_name), reg_idx, &field_name, &suffix_idx);
                                match kind {
                                    FieldHwKind::WriteEn(_)  => if_then.push((ctrl_id, field_sig.clone())),
                                    FieldHwKind::WriteEnL(_) => if_then.push((LogicExpr::not(ctrl_id), field_sig.clone())),
                                    FieldHwKind::Set(_) => {
                                        let val = if field.width == 1 {
                                            LogicExpr::ValueU(1, 1)
                                        } else {
                                            LogicExpr::or_b(field_id.clone().into(), field_sig.clone())
                                        };
                                        if_then.push((ctrl_id, val));
                                    },
                                    FieldHwKind::Clear(info) => {
                                        let val = if field.width == 1 {
                                            LogicExpr::ValueU(0, 1)
                                        } else {
                                            LogicExpr::and_b(field_id.clone().into(), LogicExpr::not_b(field_sig.clone()))
                                        };
                                        if_then.push((ctrl_id, val));
                                    },
                                    FieldHwKind::Toggle(info) => {
                                        let val = if field.width == 1 {
                                            LogicExpr::not_b(field_id.clone().into())
                                        } else {
                                            LogicExpr::or_b(field_id.clone().into(), field_sig.clone())
                                        };
                                        if_then.push((ctrl_id, val));
                                    },
                                    // Nothing todo for other HwKind (already handled for interrup, counter has less prevalence than software access)
                                    FieldHwKind::Counter(_) |
                                    FieldHwKind::Interrupt(_) |
                                    FieldHwKind::ReadOnly => {},
                                }
                            }
                        }

                        // Handle Software access
                        if field.is_sw_write() {
                            let mut cond = LogicExpr::and(decode.clone(), rif_en.clone());
                            if field.sw_kind==FieldSwKind::ReadClr {
                                cond.push(rd_wrn.clone());
                            } else {
                                cond.push(LogicExpr::not(rd_wrn.clone()));
                            }
                            let val = match &field.sw_kind {
                                // Basic write
                                FieldSwKind::ReadWrite |
                                FieldSwKind::WriteOnly => wr_data,
                                //
                                FieldSwKind::ReadClr => {
                                    if field.is_signed() {
                                        LogicExpr::ValueI(0, field.width.into())
                                    } else {
                                        LogicExpr::ValueU(0, field.width.into())
                                    }
                                }
                                // Write 1 Clear: set to 0 all bit being 1 in the write data bus
                                FieldSwKind::W1Clr => {
                                    if field.width==1 {
                                        cond.push(wr_data);
                                        LogicExpr::ValueU(0, 1)
                                    } else {
                                        LogicExpr::and_b(field_id.clone().into() , LogicExpr::not_b(wr_data))
                                    }
                                }
                                // Write 0 Clear: set to 0 all bit being 0 in the write data bus
                                FieldSwKind::W0Clr => {
                                    if field.width==1 {
                                        cond.push(LogicExpr::not(wr_data));
                                        LogicExpr::ValueU(0, 1)
                                    } else {
                                        LogicExpr::and_b(field_id.clone().into() , wr_data)
                                    }
                                }
                                // Write 1 Set or Pulse: set to 1 all bit being 1 in the write data bus
                                FieldSwKind::W1Set |
                                FieldSwKind::W1Pulse(_,_) => {
                                    if field.width==1 {
                                        wr_data
                                    } else {
                                        LogicExpr::or_b(field_id.clone().into(), wr_data)
                                    }
                                }
                                // Write 1 Toggle: flip all bits being 1 in write data bus
                                FieldSwKind::W1Tgl => {
                                    if field.width==1 {
                                        LogicExpr::not_b(field_id.clone().into())
                                    } else {
                                        LogicExpr::xor(field_id.clone().into() , wr_data)
                                    }
                                }
                                // Password:
                                //  - Set b0 to 0 when password matches expected
                                //  - Set b1 to 1 when value matches the hold password (it defined)
                                //  - Set both value to 1 when password does not match and protection is enabled (i.e. locked until reset)
                                FieldSwKind::Password(info) => {
                                    if info.protect || (info.once.is_some() && info.hold.is_some()) {
                                        let hold_not_lock = LogicExpr::or(
                                            field_id.with_fsuffix("_hold").into(),
                                            LogicExpr::not(field_id.with_fsuffix("locked").into()));
                                        cond.push(hold_not_lock);
                                    }
                                    let mut if_then_pw = Vec::new();
                                    if let Some(v) = &info.once {
                                        if_then_pw.push((
                                            LogicExpr::eq(wr_data.clone(), LogicExpr::reset(&v.into(), field.width)),
                                            LogicExpr::ValueU(0, 2)
                                        ));
                                    }
                                    if let Some(v) = &info.hold {
                                        if_then_pw.push((
                                            LogicExpr::eq(wr_data.clone(), LogicExpr::reset(&v.into(), field.width)),
                                            LogicExpr::ValueU(2, 2)
                                        ));
                                    }
                                    if info.protect {
                                        if_then_pw.push((
                                            LogicExpr::neq(wr_data.clone(), LogicExpr::ValueU(0, field.width.into())),
                                            LogicExpr::ValueU(3, 2)
                                        ));
                                    }
                                    LogicExpr::Ite(if_then_pw, LogicExpr::ValueU(1, 2).into())
                                }
                                // Read Only case should be impossible due to the is_sw_write check earlier
                                FieldSwKind::ReadOnly => unreachable!(),
                            };
                            if_then.push((cond, val));
                            // If Once password, reset value on any register write
                            if let Some(pw) = field.password_info() {
                                if pw.once.is_some() {
                                    let mut cond_once = LogicExpr::and(rif_en.clone(), LogicExpr::not(rd_wrn.clone()));
                                    if pw.hold.is_some() {
                                        cond_once.push(LogicExpr::not(field_id.with_fsuffix("_hold").into()));
                                    }
                                    if_then.push((cond_once, LogicExpr::ValueU(1, 2)));
                                }
                            }

                        }

                        // Handle Counter
                        if let Some(info) = field.counter_info() {
                            let hw_path = field_id.with_name(field_id.name.strip_prefix("rif_").unwrap_or(&field_id.name).to_owned());
                            if info.clr {
                                if_then.push((hw_path.with_fsuffix("_clr").into(), reset_expr.clone()));
                            }
                            let one =
                                if field.is_signed() {LogicExpr::ValueI(1, field.width.into())}
                                else {LogicExpr::ValueU(1, field.width.into())};
                            if info.is_up() {
                                let cond = hw_path.with_fsuffix("_incr_en").into();
                                let incr = if info.incr_val>1 {hw_path.with_fsuffix("_incr_val").into()} else {one.clone()};
                                let val = LogicExpr::add(field_id.clone().into(), incr);
                                if_then.push((cond,val));
                            }
                            if info.is_down() {
                                let cond = hw_path.with_fsuffix("_decr_en").into();
                                let decr = if info.decr_val>1 {hw_path.with_fsuffix("_decr_val").into()} else {one.clone()};
                                let val = LogicExpr::sub(field_id.clone().into(), decr);
                                if_then.push((cond,val));
                            }
                        }

                        // Default next to current value
                        let val_default = match &field.sw_kind {
                            FieldSwKind::W1Pulse(_,_) if field_clken.is_empty() => LogicExpr::ValueU(0, field.width.into()),
                            FieldSwKind::Password(info) => {
                                let b1 = if info.hold.is_some() {field_id.with_fsuffix("_hold").into()}
                                    else {LogicExpr::ValueU(0, 1)};
                                LogicExpr::Concat([b1, field_id.with_fsuffix("_locked").into()].to_vec())
                            }
                            _ => field_id.clone().into()
                        };

                        self.write_assign(field_next_id, LogicExpr::Ite(if_then, Box::new(val_default)));

                    }
                    // Handle case of partial field where one part is read-only
                    else if field_impl.has_write_mod() {
                        self.write_assign(field_next_id, LogicExpr::ValueU(0, field.width.into()));
                    }
                    // Handle case of direct feedback of Hardware written field to hardware read value
                    else if field.hw_access==Access::RW && !reg.is_intr_derived() {
                        self.write_assign(field_id, field_sig);
                    }
                }

                // External register
                if reg.is_external() {
                    let mut ext_path = "ext".to_owned();
                    if reg_impl.is_multi_ext() {
                        ext_path.push_str(&format!("_{}",reg.reg_name));
                    }
                    if reg.sw_access.is_writable() {
                        let decode_wr = LogicExpr::And([decode.clone(), rif_en.clone(), LogicExpr::not(rd_wrn.clone())].to_vec());
                        let ext_write : ExprId = sw_groupd_id.with_path(self.casing(&format!("{ext_path}_write")), None);
                        self.write_assign(ext_write, decode_wr);
                    }
                    if reg.sw_access.is_readable() {
                        let decode_rd = LogicExpr::And([decode.clone(),rif_en.clone(),rd_wrn.clone()].to_vec());
                        let ext_read : ExprId = sw_groupd_id.with_path(self.casing(&format!("{ext_path}_read")), None);
                        self.write_assign(ext_read, decode_rd);
                    }
                }
                // Sequential process
                else if reg.has_proc() {
                    let decode_en = LogicExpr::and(decode.clone(), rif_en.clone());
                    // Get a default clock for the register
                    let reg_clk =
                        if let Some(n) = &reg_impl.clk {n}
                        else if reg.sw_access.is_writable() && !reg.is_intr() {&rif.sw_clocking.clk}
                        else {&hw_clk.clk};
                    // Collect each field signal info in a hashmap indexed by a couple (clk/rst)
                    let mut signals: OrderDict<(String,String), Vec<SignalInfo> > = OrderDict::new();
                    for field in reg.fields.iter().filter(|f| !f.is_disabled() && f.partial_lsb()==0 && !f.sw_kind.is_pulse_comb()) {
                        // Skip expanded field after first index
                        if reg.array.dim_def() > 0 && reg.array.idx() > 0 && field.array.dim() == 0 {
                            continue;
                        }
                        // Get field implementation
                        let field_impl = reg_impl.get_field(&field.name)?;
                        // Skip field with no hardware
                        if !field_impl.is_hw_write() && !field_impl.is_sw_write() {
                            continue;
                        }
                        let field_name = self.casing(&field.name);
                        let field_name_flat = self.casing(&field.name_flat());
                        let field_idx = if field.array.dim() > 0 {
                            Some(SignalRange::new_bit(field.array.idx() as u8))
                        } else {None};
                        let field_cast = field_impl.hdl_cast(&rif_pkg_name, &reg.reg_type);
                        let suffix_idx = field.partial_suffix();
                        let reg_field_name = format!("{group_name}{intr_suffix}{reg_idxf}_{field_name_flat}");
                        let field_id = if field_impl.is_local() && field.has_write_mod() && !reg.is_external() {
                            ExprId::new_range(format!("{reg_field_name}__reg"), None)
                        } else if field.is_password() {
                            ExprId::new_field_range(
                                format!("rif_{group_name}{intr_suffix}"), reg_idx,
                                format!("{field_name}_locked"), None)
                        } else {
                            ExprId::new_field_range(
                                format!("rif_{group_name}{intr_suffix}"), reg_idx,
                                field_name.clone(), field_idx.clone())
                        };
                        // Get clock associated with the field
                        let f_clk =
                            if let Some(n) = &field_impl.clk {n}
                            else if let Some(n) = &reg_impl.clk {n}
                            else if field_impl.is_hw_write() && !reg.is_intr_derived() {&hw_clk.clk}
                            else {&rif.sw_clocking.clk};
                        // if reg.reg_name=="" {println!("Field {reg_name_i}.{field_name} : Kind={:?} hw_write={} -> {f_clk} | field:{:?} | reg:{:?}", field.hw_kind, field.is_hw_write(), field_impl.clk, reg_impl.clk);}
                        // Get reset associated with the field
                        let f_rst =
                            // TODO: Add optional reset name per field
                            if let Some(n) = &reg_impl.rst {n}
                            else if f_clk==&hw_clk.clk {&hw_clk.rst.name}
                            else {&rif.sw_clocking.rst.name};
                        // Next value
                        let next : ExprId = format!("{reg_field_name}__next").into();
                        let value : LogicExpr = if field.is_password() {
                            next.with_idx(0).into()
                        } else if let Some(cnt_info) = field.counter_info().and_then(|info| if info.has_satn() {Some(info)} else {None}) {
                            let cond = field_id.with_fsuffix("_event").into();
                            let sat = if field.is_signed() {
                                let wm1 = field.width - 1;
                                let w = field.width as usize;
                                let is_neg = LogicExpr::lt(next.clone().into(), LogicExpr::ValueI(0, w+1));
                                LogicExpr::ite(is_neg, LogicExpr::ValueI(-(1<<wm1), w), LogicExpr::ValueI((1<<wm1)-1, w))
                            } else {
                                let is_neg = LogicExpr::eq(next.with_idx(w).into(), LogicExpr::ValueU(1, 1));
                                LogicExpr::ite(is_neg, LogicExpr::ValueI(0, field.width.into()), LogicExpr::ValueI((1<<w)-1, field.width.into()))
                            };
                            LogicExpr::ite(cond, sat, next.clone().into())
                        } else {
                            next.clone().into()
                        };
                        // Enable
                        let enable = if reg.is_intr() {
                            format!("clk_en_intr_{group_name}")
                        } else if let ClkEn::Signal(clk_en) = &field_impl.clk_en {
                            clk_en.clone()
                        } else if let ClkEn::Signal(clk_en) = &reg_impl.clk_en {
                            clk_en.clone()
                        } else if field.is_hw_write() {
                            hw_clk.en.clone()
                        } else {
                            rif.sw_clocking.en.clone()
                        };
                        let mut enable_expr : Option<LogicExpr> = if enable.is_empty() {None} else {Some(enable.clone().into())};
                        // Prevent counter update on saturation when increment/decrement is 1
                        if let Some(cnt_info) = field.counter_info() {
                            if cnt_info.sat && cnt_info.incr_val <= 1 && cnt_info.decr_val <= 1 {
                                let sat = group_id.with_path(format!("{field_name}_event"), field_idx);
                                let not_sat = LogicExpr::not(sat.into());
                                if let Some(e) = &mut enable_expr {
                                    *e = LogicExpr::and((*e).clone(), not_sat);
                                } else {
                                    enable_expr = Some(not_sat);
                                }
                            }
                        }
                        // Use the local enable (or-ed with interface enable) if register can be modifed by firmware access
                        if enable_expr.is_some() && enable == hw_clk.en && field.is_sw_write() {
                            enable_expr = enable_expr.map(|e| LogicExpr::or(e, decode_en.clone()));
                        }
                        // Prevent modification when lock signal is high
                        if field_impl.lock.is_some() {
                            let path = field_impl.lock.expr();
                            let lock_id = path_to_signal(path, "_lock", (&reg.group_type, &group_name), reg_idx, &field_name, &suffix_idx);
                            let not_lock = LogicExpr::not(lock_id);
                            if let Some(e) = &mut enable_expr {
                                *e = LogicExpr::and((*e).clone(), not_lock);
                            } else {
                                enable_expr = Some(not_lock);
                            }
                        }
                        // Clear
                        let clear : Option<LogicExpr> = if field_impl.clear.is_some() {
                            Some(path_to_signal(&field_impl.clear, "_clr", (&reg.group_type,  &group_name), reg_idx, &field.name, &suffix_idx))
                        } else if reg_impl.clear.is_some() {
                            Some(path_to_signal(&reg_impl.clear, "reg_clr", (&reg.group_type,  &group_name), reg_idx, "", ""))
                        } else {
                            None
                        };
                        // TBD: handle case where a clear is defined for the clock of this field ?
                        // Reset
                        let reset = if field.is_password() {
                            LogicExpr::ValueU(1, 1)
                        } else {
                            let field_reset = if field.is_partial() {
                                let rst_val = field_impl.get_reset(reg.group_idx);
                                if field_impl.signed {
                                    LogicExpr::ValueI(rst_val as i128, field_impl.width.into())
                                } else {
                                    LogicExpr::ValueU(rst_val, field_impl.width.into())
                                }
                            } else {
                                LogicExpr::reset(&field.reset, field_impl.width as u8)
                            };
                            if field_cast.is_custom() {
                                LogicExpr::Cast(field_cast.clone(), Box::new(field_reset))
                            } else {
                                field_reset
                            }
                        };
                        // Add the signal info the hashmap
                        let k = (f_clk.to_string(),f_rst.to_string());
                        let field_entry = signals.entry(&k);

                        field_entry.push(
                            SignalInfo::new_with_en_clr(field_id.clone(), reset, value, enable_expr.clone(), clear.clone())
                        );

                        // For password protected or with both option once/hold, add another signal
                        if let FieldSwKind::Password(info) = &field.sw_kind {
                            if info.has_hold() {
                                field_entry.push(SignalInfo::new_with_en_clr(
                                    field_id.with_fsuffix("_hold"),
                                    LogicExpr::ValueU(0, 1),
                                    next.with_idx(1).into(),
                                    enable_expr, clear)
                                );
                            }
                        }
                        // For interrupt on edge, add delay version of the interrupt event
                        else if let Some(FieldHwKind::Interrupt(info)) = field.hw_kind.first() {
                            if !info.is_level() {
                                field_entry.push(SignalInfo::new_with_en_clr(
                                    field_id.with_name(format!("{group_name}{intr_suffix}_d1")),
                                    LogicExpr::ValueU(0, 1),
                                    field_id.with_name(format!("{group_name}{intr_suffix}_l")).into(),
                                    enable_expr, clear)
                                );
                            }
                        }
                    }
                    // Create one process for each pair of clock/reset found in the register field
                    for ((clk,rst_name),sig_list) in signals.items() {
                        let mut proc_name = format!("proc_{reg_name}");
                        // Append clk/rst_name to process if different from the register default
                        if clk!=reg_clk && signals.len() > 1 {
                            proc_name.push_str(&format!("_{clk}"));
                        }
                        let mut rst = if clk==&rif.sw_clocking.clk || rif.hw_clocking.is_empty() {&rif.sw_clocking.rst} else {&rif.hw_clocking.first().unwrap().rst};
                        // Find the full reset definition in the sw_clock or hw_clocking
                        if rst_name!=&rst.name {
                            proc_name.push_str(&format!("_{rst_name}"));
                            if rst_name == &rif.sw_clocking.rst.name {
                                rst = &rif.sw_clocking.rst;
                            } else {
                                rst = &rif.hw_clocking.iter()
                                    .find(|&x| &x.rst.name==rst_name)
                                    .ok_or(format!("Reset {rst_name} should be amongst the software or hardware reset list !"))?
                                    .rst;
                            }
                        }
                        //
                        self.write_process_seq(clk, rst, &proc_name, sig_list);
                    }
                }

                // Create process to generate register pulse access
                let group_name_i = reg.group_name_i(); // Group name with optional index in bracket
                if !group_done.contains(&group_name_i) {
                    group_done.insert(group_name_i.clone());
                    let mut signals: Vec<SignalInfo> = Vec::new();
                    let mut reg_clk = "".to_owned();
                    for ctrl in reg_impl.regs_ctrl.iter() {
                        let ctrl_name = format!("{}{reg_idxf}",ctrl.name.to_casing(Snake));
                        let base_name = if reg_impl.is_multi_pulse() { format!("p_{ctrl_name}")} else {"p".to_owned()};
                        let base_value = LogicExpr::and(
                            format!("{ctrl_name}__decode").into(),
                            rif_en.clone());
                        for pulse in ctrl.pulse.iter() {
                            let mut name = base_name.to_owned();
                            let mut value = base_value.clone();
                            match pulse {
                                RegPulseKind::Write(clk)  => {
                                    name.push_str("_write");
                                    value.push(LogicExpr::not(rd_wrn.clone()));
                                },
                                RegPulseKind::Read(clk)   => {
                                    name.push_str("_read");
                                    value.push(rd_wrn.clone());
                                },
                                RegPulseKind::Access(clk) => name.push_str("_access"),
                            };
                            // No clock means the pulse is just combinatorial logic
                            let p_clk = pulse.clk();
                            if p_clk.is_empty() {
                                self.write_assign(sw_groupd_id.with_path(name, None), value);
                            } else {
                                if reg_clk.is_empty() {
                                    reg_clk = p_clk.to_owned();
                                } else if reg_clk!=p_clk {
                                    return Err(format!("Only one clock should be used for the register {group_name} pulses").into());
                                }
                                signals.push(SignalInfo::new(sw_groupd_id.with_path(name, None), LogicExpr::ValueU(0,1), value));
                            }
                        }
                    }
                    if !signals.is_empty() {
                        let proc_name = format!("proc_{group_name}{reg_idxf}_special");
                        self.write_process_seq(&reg_clk, &rif.sw_clocking.rst, &proc_name, &signals);
                    }
                }

                // Interrupt registers signals : clock enable and IRQ
                if reg.is_intr() {

                    let intr_info = reg_impl.intr_info(reg)?;
                    // Enable clock of interrupt register when there is an event or when accessed from RIF
                    // Replace the event by the register clock enable if defined
                    let mut intr_en : Vec<LogicExpr> = Vec::new();
                    if let ClkEn::Signal(clk_en) = &reg_impl.clk_en {
                        intr_en.push(clk_en.as_str().into());
                    } else {
                        for field in reg.fields.iter() {
                            let field_name = self.casing(&field.name());
                            let intr_l : LogicExpr = format!("{group_name}_l.{field_name}").into();
                            let intr_val: LogicExpr = match field.intr_trig(intr_info.trigger) {
                                InterruptTrigger::High => LogicExpr::ValueU(0, field.width.into()),
                                InterruptTrigger::Low  => LogicExpr::ValueU((1<<field.width) - 1, field.width.into()),
                                _ => format!("{group_name}_d1.{field_name}").into()
                            };
                            intr_en.push(LogicExpr::neq(intr_l, intr_val));
                        }
                    }
                    intr_en.push(rif_en.clone());
                    let clk_en_intr = format!("clk_en_intr_{group_name}").into();
                    self.write("\n");
                    self.write_assign(clk_en_intr, LogicExpr::Or(intr_en));
                    self.write("\n");

                    // Pending signal:  interrupts field status and-ed with mask
                    let mut pendings : Vec<LogicExpr> = Vec::with_capacity(reg.fields.len());
                    for field in reg.fields.iter() {
                        let field_name = self.casing(&field.name());
                        let rhs : LogicExpr = if field.is_disabled() {
                            LogicExpr::ValueU(0, field.width.into())
                        } else if intr_info.mask.is_some() {
                            LogicExpr::and_b(
                                format!("rif_{group_name}.{field_name}").into(),
                                format!("rif_{group_name}_mask.{field_name}").into())
                        } else {
                            format!("rif_{group_name}.{field_name}").into()
                        };
                        let pending : ExprId = format!("rif_{group_name}_pending.{field_name}").into();
                        if !field.is_disabled() {
                            if field.width > 1 {
                                pendings.push(LogicExpr::neq(pending.clone().into(), LogicExpr::ValueU(0, field.width.into())));
                            } else {
                                pendings.push(pending.clone().into());
                            }
                        }
                        self.write_assign(pending, rhs);
                    }

                    // IRQ: or of all pending interrupts
                    self.write("\n");
                    let irq = format!("rif_{group_name}_irq").into();
                    self.write_assign(irq, LogicExpr::Or(pendings));
                    self.write("\n");
                }

                // Concatenation for Read data
                let fields = reg.fields.iter().rev().filter(|f| !f.sw_kind.is_wo());
                let nb_fields = fields.clone().count();
                let first_field = reg.fields.iter().find(|f| !f.sw_kind.is_wo());
                let first_width = if let Some(f) = first_field {f.width} else {0};
                let first_is_signed = if let Some(f) = first_field {f.is_signed()} else {false};

                // Track previous LSB to know when to inssert zero-padding
                let mut prev_lsb = rif.data_width;
                // pre-allocate vector with one more element to handle the basic case where
                // whole register is not full and at least one padding will be neccessary
                let mut values : Vec<LogicExpr> = Vec::with_capacity(nb_fields+1);
                for field in fields {
                    let field_impl = reg_impl.get_field(&field.name)?;
                    let field_name = field.name.to_casing(Snake);
                    // Fill register spaces with 0s
                    let spaces = prev_lsb.saturating_sub(field.msb()+1);
                    if spaces != 0 {
                        values.push(LogicExpr::ValueU(0, spaces.into()));
                    }
                    //
                    if !reg.is_external() && field_impl.is_local() && field.has_write_mod() {
                        let partial  = field.to_range(reg_def_idx,false);
                        let name = self.casing(&format!("{group_name}{intr_suffix}{reg_idxf}_{}__reg", field.name_flat()));
                        values.push(ExprId::new_range(name, partial.clone()).into());
                    } else if let FieldSwKind::Password(info) = &field.sw_kind {
                        if info.has_hold() {
                            values.push(LogicExpr::ValueU(0, (field.width-2).into()));
                            let hold : ExprId = sw_groupd_id.with_path(self.casing(&format!("{}_hold", field.name)), None);
                            values.push(hold.into());
                        } else {
                            values.push(LogicExpr::ValueU(0, (field.width-1).into()));
                        }
                        let lock : ExprId = sw_groupd_id.with_path(self.casing(&format!("{}_locked", field.name)), None);
                        values.push(lock.into());
                    } else {
                        let prefix = if !reg.is_external() && (field_impl.is_sw_write() || field.is_hw_write() || field_impl.is_constant()) {"rif_"} else {""};
                        let name = format!("{prefix}{group_name}{intr_suffix}");
                        let field_range = field.to_range(reg_def_idx,true);
                        let value = ExprId::new_field_range(name, reg_idx, field_name.to_owned(), field_range);
                        if let EnumKind::Type(n) = &field.enum_kind {
                            values.push(LogicExpr::CastFrom(n.to_owned(), field.width, Box::new(value.into())));
                        } else {
                            values.push(value.into());
                        }
                    }
                    // Save LSB
                    prev_lsb = field.lsb;
                }
                if prev_lsb != 0 {
                    values.push(LogicExpr::ValueU(0, prev_lsb.into()));
                }
                let is_single_wide_field = nb_fields==0 || (nb_fields == 1 && first_width==rif.data_width);
                let rhs : LogicExpr = if is_single_wide_field {
                    let expr = values.first().cloned().unwrap_or(LogicExpr::ValueU(0, rif.data_width.into()));
                    if first_is_signed {
                        LogicExpr::Cast(CastInfo::Unsigned, Box::new(expr))
                    } else {
                        expr
                    }
                } else {
                    LogicExpr::Concat(values)
                };
                let rd_data : ExprId = format!("{reg_name}__read_data").into();
                self.write_assign(rd_data.clone(), rhs);

                // For optional register close the generate part and assign the read_data to 0
                if reg.optional.is_some() {
                    self.write_generate_else(format!("gen_noreg_{reg_name}"));
                    self.write_assign(rd_data, LogicExpr::ValueU(0, rif.data_width.into()));
                    self.write_generate_end(format!("gen_reg_{reg_name}"));
                }
            }
        }

        // Handle case of missing fields in a register implementation
        for (group_name, hw_reg) in rif.hw_regs.items() {
            // Skip register if read-only from firmware
            let reg_impl = rif.get_hw_reg(&hw_reg.group);
            if !reg_impl.port.is_out() && reg_impl.interrupt.is_empty() {continue;}
            for (field_name,info) in &hw_reg.missing_fields {
                let name = format!("rif_{group_name}.{field_name}");
                self.write_assign(name.into(), info.into());
            }
        }


        // Close the module
        self.write_module_impl_footer(&rif_name);

        // Save file
        let filename = self.filename_rif(rif);
        self.save(&filename)
    }

    // Add a clocking port to a module interface
    fn add_clocking_port(&mut self, info: &ClockingInfo, list: &mut HashSet<String>, is_hw: bool) {
        let kind = if is_hw { "Hardware" } else { "Software" };
        // Clock
        if !list.contains(&info.clk) {
            let port = PortInfo::new_in(info.clk.to_owned(), format!("{kind} clock"));
            self.write_port_decl(&port, None, false);
            list.insert(info.clk.to_owned());
        }
        // Reset
        if !list.contains(&info.rst.name) {
            let port = PortInfo::new_in(info.rst.name.to_owned(), format!("{kind} {}", info.rst.desc()));
            self.write_port_decl(&port, None, false);
            list.insert(info.rst.name.to_owned());
        }
        // Clear
        if !info.clear.is_empty() && !list.contains(&info.clear) {
            let port = PortInfo::new_in(info.clear.to_owned(), format!("{kind} clear"));
            self.write_port_decl(&port, None, false);
            list.insert(info.clear.to_owned());
        }
    }

    /// Generate RIFmux package (define address constant for each RIF instance)
    fn gen_rifmux_pkg(&mut self, rifmux: &RifmuxInst) -> Result<(), Box<dyn std::error::Error>> {
        let name_len = rifmux.components.iter().map(|c| c.get_name().len()).max().unwrap_or(0);
        self.write_file_header();
        self.write_rifmux_pkg_header(rifmux);
        let w = ((rifmux.addr_width+3)>>2) as usize;
        for comp in rifmux.components.iter() {
            let pad = name_len - comp.get_name().len();
            let name = format!("{}_BASE_ADDR{:<pad$}", comp.get_name().to_uppercase(), "");
            let decl: SignalDecl= SignalDef::new_bus(name, rifmux.addr_width as u16, false).into();
            let val = LogicExpr::ValueU(comp.addr.into(), rifmux.addr_width.into());
            self.write_const(&decl, val);
        }
        self.write("\n");
        self.write_rifmux_pkg_footer(rifmux);

        // Write file
        let filename = self.filename_rifmux_pkg(rifmux);
        self.save(&filename)
    }

    // Hooks for RIF mux package
    fn write_rifmux_pkg_header(&mut self, rifmux: &RifmuxInst) {
        self.write_pkg_header(&rifmux.type_name);
    }
    fn write_rifmux_pkg_footer(&mut self, rifmux: &RifmuxInst) {
        self.write_pkg_footer(&rifmux.type_name);
    }

    /// Generate RIFmux module
    fn gen_rifmux(&mut self, rifmux: &RifmuxInst) -> Result<(), Box<dyn std::error::Error>> {
        let rifmux_name = self.casing(&rifmux.type_name);
        let name_len = rifmux.components.iter().map(|c| c.get_name().len()).max().unwrap_or(0);
        self.set_comp(rifmux.into(), true);
        self.set_rifmux_info(rifmux);
        self.write_file_header();
        self.write_module_decl_header(&rifmux_name);
        self.write_module_port_header();
        // Add port/reset port if not default interface
        if !rifmux.interface.is_default() {
            self.write_port_decl(&PortInfo::new_in(
                rifmux.sw_clocking.clk.to_owned(), "Bridge clock".to_owned()), None, false);
            self.write_port_decl(&PortInfo::new_in(
                rifmux.sw_clocking.rst.name.to_owned(),
                format!("Bridge reset : {}", rifmux.sw_clocking.rst.desc())), None, false);
        }
        // Add RIF interface for each component
        for comp in rifmux.components.iter() {
            let port = PortInfo::new_intf(
                format!("if_{:<1$}", comp.get_name(), name_len),
                "rif_if".to_owned(), "ctrl".to_owned(),
                comp.get_desc_short(false));
            self.set_addr_width(comp.get_addr_width());
            self.write_port_decl(&port, None, false);
        }
        self.set_addr_width(rifmux.addr_width);
        // Add top interface and close module declaration
        self.write_intf_ports(&rifmux.interface);
        self.write_module_decl_footer(&rifmux_name);
        self.write("\n");
        // Signals declaration
        self.write_signal_decl(&SignalDecl::new_bit(
            "addr_invalid".to_owned(),
            "High when address is not in the range of any of the connected RIF".to_owned()));
        self.write_signal_decl(&SignalDecl::new_bit(
            "addr_invalid_next".to_owned(),
            "Combinatorial version of addr_invalid".to_owned()));

        let comp_info : CompInfo = rifmux.into();
        if !rifmux.interface.is_default() {
            self.write_rif_decl(&comp_info, true, &rifmux.sw_clocking.clk, &rifmux.sw_clocking.rst.name);
        }
        self.write_signal_decl_footer(false);
        // Add interface bridge when not default
        self.write_intf_bridge(&rifmux.interface, &comp_info, &rifmux.sw_clocking.clk, &rifmux.sw_clocking.rst.name);

        // Address demultiplexing
        self.write_comment_box("Demux access");
        let msb = rifmux.addr_width - 1;
        let mut en_names : Vec<LogicExpr> = Vec::new();
        let rif_addr : ExprId = ("if_rif","addr").into();
        for comp in rifmux.components.iter() {
            let name = comp.get_name();
            let width = comp.get_addr_width();
            let range = Some(SignalRange::new(width, msb));
            let addr_v = LogicExpr::eq(
                ("if_rif", "addr", range).into(),
                LogicExpr::ValueU((comp.addr >> width) as u128, (msb-width+1) as usize)
            );
            self.write_comment(1, &name.to_casing(Casing::Title));
            let pad = name_len + 7 - name.len();
            // Enable: rif_en & addr_v
            let en : ExprId = (format!("if_{name}"),"en".to_owned()).into();
            self.write_assign(
                en.with_path(format!("{:<pad$}","en"), None),
                LogicExpr::And(vec![("if_rif","en").into(), addr_v.clone()]));
            // Force address to 0 when not valid
            self.write_assign(
                en.with_path(format!("{:<pad$}","addr"), None),
                LogicExpr::ite(
                    addr_v,
                    rif_addr.with_range(SignalRange::new(0, width-1)).into(),
                    LogicExpr::ValueU(0, width.into())
                )
            );
            // Direct copy data & rd_wrn on the interface
            self.write_assign(
                en.with_path(format!("{:<pad$}","wr_data"), None),
                ("if_rif","wr_data").into());
            self.write_assign(
                en.with_path(format!("{:<pad$}","rd_wrn"), None),
                ("if_rif","rd_wrn").into());
            // Save enable list for later
            en_names.push(en.into());
        }

        // Feedback (read data/error) multiplexing
        self.write_comment_box("Mux feedback");
        self.write_assign(
            "addr_invalid_next".into(),
            LogicExpr::and(
                ("if_rif","en").into(),
                LogicExpr::not(LogicExpr::Or(en_names))
            )
        );
        // TODO : Use argument to insert pipe
        self.write_assign(
            "addr_invalid".into(),
            "addr_invalid_next".into()
        );
        self.write("\n");

        let mut dones : Vec<LogicExpr> = ["addr_invalid".into()].to_vec();
        dones.extend(rifmux.components.iter()
            .map(|c| {
                let id : ExprId = (
                    format!("if_{}", c.get_name()),
                    format!("{0:<1$}", "done", name_len-c.get_name().len()+4)
                    ).into();
                LogicExpr::from(id)
            })
        );
        self.write_assign(("if_rif","done").into(), LogicExpr::Or(dones));

        let mut dones : Vec<LogicExpr> = ["addr_invalid_next".into()].to_vec();
        dones.extend(rifmux.components.iter()
            .map(|c| {
                let id : ExprId = (
                    format!("if_{}", c.get_name()),
                    format!("{0:<1$}", "done_next", name_len-c.get_name().len()+9)
                    ).into();
                LogicExpr::from(id)
            })
        );
        self.write_assign(("if_rif","done_next").into(), LogicExpr::Or(dones));
        self.write("\n");

        self.write_rifmux_muxfb(&rifmux.components, "rd_data", name_len, (0, rifmux.data_width.into()));
        self.write_rifmux_muxfb(&rifmux.components, "err_addr", name_len, (1,1));
        self.write_rifmux_muxfb(&rifmux.components, "err_access", name_len, (0,1));
        self.write_rifmux_muxfb(&rifmux.components, "err_addr_next", name_len, (1,1));
        self.write_rifmux_muxfb(&rifmux.components, "err_access_next", name_len, (0,1));

        self.write_module_impl_footer(&rifmux_name);

        // Write file
        self.save(&format!("{}.{}", rifmux_name, Self::EXT))
    }

    fn write_rifmux_muxfb(&mut self, comps: &[CompInst], name: &str, len: usize, err_val: (usize,usize)) {
        let suffix = if name.ends_with("_next") {"_next"} else {""};
        let mut rhs = LogicExpr::Ite(Vec::new(), Box::new(LogicExpr::ValueU(err_val.0 as u128, err_val.1)));
        for (i,comp) in comps.iter().enumerate() {
            let top : ExprId = format!("if_{}",comp.get_name()).into();
            let pad = len - comp.get_name().len();
            rhs.add_ite(
                top.with_path(format!("done{0:<1$}", suffix, pad+suffix.len()), None).into(),
                top.with_path(format!("{0:<1$}", name, pad+name.len()), None).into(),
            );
        }
        self.write_assign(("if_rif",name).into(),rhs);
    }

    fn gen_riftop(&mut self, rifmux: &RifmuxInst) -> Result<(), Box<dyn std::error::Error>> {
        let Some(riftop) = &rifmux.top else { return Ok(())};
        let riftop_name = self.casing(&riftop.name);

        let sw_clk = &rifmux.sw_clocking.clk;
        let sw_rst = &rifmux.sw_clocking.rst.name;
        let intf_ports = RifIntfPorts::new(&rifmux.interface, Self::SUPPORT_INTF);
        let mut names : Vec<String> = [sw_clk.to_owned(), sw_rst.to_owned()].to_vec();
        self.set_comp(rifmux.into(), true);
        self.set_rifmux_info(rifmux);
        self.write_file_header();
        // Module declaration
        self.write_module_decl_header(&riftop_name);
        self.write_module_port_header();
        self.write_comment(1, "RTL clock/reset");
        self.write_port_decl(&PortInfo::new_in(sw_clk.to_owned(), "Software clock".to_owned()), None, false);
        self.write_port_decl(&PortInfo::new_in(sw_rst.to_owned(), format!("Software reset : {}", rifmux.sw_clocking.rst.desc())), None, false);
        let mut nb_ctrl = 0;
        for rif in rifmux.components.iter().filter_map(|c| c.get_rif()) {
            nb_ctrl += rif.ports.clk_ens.len() + rif.ports.ctrls.len();
            let ports = rif.ports.clocks.iter().skip(1)
                .chain(rif.ports.resets.iter().skip(1));
            self.set_addr_width(rif.addr_width);
            for port in ports {
                if !names.iter().any(|n| n==port.name()) {
                    names.push(port.name().to_owned());
                    self.write_port_decl(port, None, false);
                }
            }
        }
        // Controls: clock enables, clear, lock, ...
        if nb_ctrl > 0 {
            self.write_comment(1, "Control signals");
            for rif in rifmux.components.iter().filter_map(|c| c.get_rif()) {
                let ports = rif.ports.clk_ens.iter()
                    .chain(rif.ports.ctrls.iter());
                for port in ports {
                    if !names.iter().any(|n| n==port.name()) {
                        names.push(port.name().to_owned());
                        self.write_port_decl(port, None, false);
                    }
                }
            }
        }
        // Register of each instances
        for rif in rifmux.components.iter().filter_map(|c| c.get_rif()) {
            self.set_addr_width(rif.addr_width);
            let prefix = riftop.prefixes.get(&rif.inst_name);
            self.write_comment(1, &format!("{} registers", rif.inst_name.to_casing(Casing::Title)));
            for port in rif.ports.regs.iter().filter(|p| p.dir.is_in()) {
                self.write_port_decl(port, prefix, false);
            }
            for port in rif.ports.regs.iter().filter(|p| p.dir.is_out()) {
                self.write_port_decl(port, prefix, false);
            }
            for port in rif.ports.irqs.iter() {
                self.write_port_decl(port, prefix, false);
            }
        }
        // Control interface
        self.write_comment(1, "Control interface");
        self.set_addr_width(rifmux.addr_width);
        self.write_intf_ports(&rifmux.interface);
        self.write_module_decl_footer(&riftop_name);

        // Interfaces declaration
        self.write_comment_box("Interfaces to sub-RIF");
        for comp in rifmux.components.iter().filter(|c| !c.is_external()) {
            self.set_addr_width(comp.get_addr_width());
            self.write_rif_decl(&CompInfo::from(&comp.inst), false, sw_clk, sw_rst);
        }

        // Instances RIF MUX and all RIFs
        self.write_comment_box("Instances");
        let comp_params = Vec::new();
        self.write_inst_header(&rifmux.type_name, "rifmux", &comp_params);
        self.write_port_bind(sw_clk, sw_clk, false);
        self.write_port_bind(sw_rst, sw_rst, false);
        let intf_ports = RifIntfPorts::new(&rifmux.interface, Self::SUPPORT_INTF);
        for port in intf_ports.iter() {
            self.write_port_bind(port.name(), port.name(), false);
        }
        let mut rifs = rifmux.components.iter().filter(|c| !c.is_external()).peekable();
        while let Some(rif) = rifs.next() {
            let name = format!("if_{}", rif.get_name());
            self.write_port_bind(&name, &name, rifs.peek().is_none());
        }

        //
        // RIFs
        for rif in rifmux.components.iter().filter_map(|c| c.get_rif()) {
            let inst_name = self.casing(&rif.inst_name);
            let type_name = self.casing(&rif.type_name);
            self.write_inst_header(&type_name, &inst_name, &comp_params);
            // Bind main clock/reset
            if let Some(clk) = rif.ports.clocks.first() {
                self.write_port_bind(clk.name(), sw_clk, false);
            }
            if let Some(rst) = rif.ports.resets.first() {
                self.write_port_bind(rst.name(), sw_rst, false);
            }
            // Bind all other ports
            let prefix = riftop.prefixes.get(&inst_name)
                .map(|p| format!("{p}_"))
                .unwrap_or("".to_owned());
            let ports = rif.ports.clocks.iter().skip(1)
                .chain(rif.ports.resets.iter().skip(1))
                .chain(rif.ports.clk_ens.iter())
                .chain(rif.ports.ctrls.iter());
            for port in ports {
                self.write_port_bind(port.name(), port.name(), false);
            }
            // Add prefix to input rif signal
            for port in rif.ports.regs.iter().filter(|p| p.dir.is_in()) {
                self.write_port_bind(port.name(), &format!("{prefix}{}", port.name()), false);
            }
            // Output port are prefixed by rif_: insert the prefix
            let ports = rif.ports.regs.iter().filter(|p| p.dir.is_out())
                .chain(rif.ports.irqs.iter());
            for port in ports {
                let name_base = port.name().strip_prefix("rif_").unwrap_or(port.name());
                self.write_port_bind(port.name(), &format!("rif_{prefix}{name_base}"), false);
            }
            self.write_intf_bind(&inst_name, true);
        }

        self.write_module_impl_footer(&riftop_name);

        // Write file
        self.save(&format!("{}.{}", riftop_name, Self::EXT))
    }

    fn write_intf_ports(&mut self, intf: &Interface) {
        let intf_ports = RifIntfPorts::new(intf, Self::SUPPORT_INTF);
        let mut ports = intf_ports.iter().peekable();
        while let Some(port) = ports.next()  {
            self.write_port_decl(port, None, ports.peek().is_none());
        }
    }

    fn write_intf_bridge(&mut self, intf: &Interface, comp: &CompInfo, sw_clk: &str, sw_rst: &str) {
        if intf.is_default() {
            return;
        }

        self.write_comment_box("Bridge to the internal register interface");
        self.write("\n");
        let name = format!("bridge_{}_rif", intf.name());
        let params = [
            ("ADDR_W".to_owned(), comp.addr_width as isize),
            ("DATA_W".to_owned(), comp.data_width  as isize)].to_vec();
        self.write_inst_header(&name, "bridge", &params);
        self.write_port_bind("clk"  , sw_clk, false);
        self.write_port_bind("rst_n", sw_rst, false);
        if Self::SUPPORT_IMPL_BIND {
            self.write_port_bind("*", "", true);
        } else {
            let intf_ports = RifIntfPorts::new(intf, Self::SUPPORT_INTF);
            for port in intf_ports.iter() {
                self.write_port_bind(port.name(), port.name(), false);
            }
            self.write_intf_bind("rif", true);
        }
    }

    /// Save information for the current RIF
    fn set_rif_info(&mut self, rif: &RifInst) {}
    fn set_rifmux_info(&mut self, rifmux: &RifmuxInst) {}

    //----------------------------------
    // Generic functions
    fn write_pkg_header(&mut self, name: &str) {}
    fn write_pkg_footer(&mut self, name: &str) {}
    fn write_const(&mut self, signal: &SignalDecl, value: LogicExpr) {}
    fn write_enum_header(&mut self, name: &str, width: u8) {}
    fn write_enum_entry(&mut self, entry: &EnumEntry, is_last: bool) {}
    fn write_enum_footer(&mut self, name: &str, width: u8) {}
    fn write_struct_header(&mut self, name: &str, fields: &[SignalDecl]) {}
    fn write_struct_field(&mut self, field: &SignalDecl, is_last: bool) {}
    fn write_struct_footer(&mut self, name: &str) {}
    fn write_module_generic_header(&mut self) {}
    fn write_module_generic_decl(&mut self, name: &str, range: &GenericRange, is_last: bool) {}
    fn write_module_decl_header(&mut self, name: &str) {}
    fn write_module_decl_footer(&mut self, name: &str) {}
    fn write_module_port_header(&mut self) {}
    fn write_signal_decl_footer(&mut self, is_rif: bool) {}
    fn write_module_impl_footer(&mut self, name: &str) {}
    fn write_port_decl(&mut self, port: &PortInfo, prefix: Option<&String>, is_last: bool) {}
    fn write_rif_decl(&mut self, comp: &CompInfo, single: bool, clk: &str, rst: &str) {}
    fn write_signal_decl(&mut self, signal: &SignalDecl) {}
    fn write_inst_header(&mut self, type_name: &str, inst_name: &str, params: &[(String, isize)]) {}
    fn write_port_bind(&mut self, port_name: &str, signal_name: &str, is_last: bool) {}
    fn write_intf_bind(&mut self, name: &str, is_last: bool) {}
    fn write_assign(&mut self, lhs: ExprId, rhs: LogicExpr) {}
    fn write_process_seq(&mut self, clk: &str, rst: &ResetDef, name: &str, signals: &[SignalInfo]) {}
    fn write_process_comb_header(&mut self, name: &str) {}
    fn write_match_header(&mut self, name: &str) {}
    fn write_match_footer(&mut self) {}
    fn write_match_case_header(&mut self, value: LogicExpr) {}
    fn write_match_case_footer(&mut self) {}
    fn write_cond_if(&mut self, lvl: usize, cond: LogicExpr) {}
    fn write_cond_else(&mut self, lvl: usize, cond: Option<LogicExpr>) {}
    fn write_cond_end(&mut self, lvl: usize) {}
    fn write_generate_if(&mut self, cond: LogicExpr, name: String) {}
    fn write_generate_else(&mut self, name: String) {}
    fn write_generate_end(&mut self, name: String) {}
    fn write_assign_comb(&mut self, lvl: usize, lhs: ExprId, rhs: LogicExpr) {}
    fn write_process_comb_footer(&mut self, name: &str) {}

    /// Write a comment
    fn write_comment(&mut self, lvl: usize, txt: &str);

    /// Write a comment enclosed in an ascii box
    fn write_comment_box(&mut self, txt: &str);

    /// Return number of pipe level at the interface
    fn nb_pipe(&self) -> usize {1}

    /// Filename for RIF package
    fn filename_rif_pkg(&self, rif: &RifInst) -> String {
        format!("{}_pkg.{}", rif.name(true).to_lowercase(), Self::EXT)
    }

    fn filename_rifmux_pkg(&self, rifmux: &RifmuxInst) -> String {
        format!("{}_pkg.{}", &rifmux.inst_name.to_lowercase(), Self::EXT)
    }

}

fn path_to_signal(expr: &Option<LogicExpr>, ext: &str, group_info: (&str, &str), idx: Option<u16>, field_name: &str, field_idx: &str) -> LogicExpr {
    match expr {
        None => LogicExpr::Id(ExprId {
            name: group_info.1.to_owned(),
            idx,
            field: Some(format!("{field_name}{ext}{field_idx}")),
            range: None,
        }),
        Some(e) => {
            // println!("{e:?} -> local_field={:?}, port={:?}",  e.local_field(group_info.0), e.port_name());
            if let Some(f) = e.local_field(group_info.0) {
                LogicExpr::Id((group_info.1, f).into())
            } else if let Some(n) = e.port_name() {
                LogicExpr::Id(n.into())
            } else {
                e.to_owned()
            }
        }
    }
}