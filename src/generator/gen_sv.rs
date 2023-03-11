use std::{
    collections::HashSet,
    fs::create_dir_all,
    path::PathBuf
};

use crate::{
    comp::{
        comp_inst::{ArrayIdx, Comp, CompInst, RifFieldInst, RifInst, RifmuxInst},
        hw_info::{PortDir, PortInfo, PortWidth, RifIntfPorts, SignalInfo}},
    rifgen::{
        order_dict::OrderDict, Access, ClkEn, ClockingInfo, CounterInfo, CounterKind, EnumKind, ExternalKind, FieldHwKind, FieldSwKind, Interface, InterruptClr, InterruptRegKind, InterruptTrigger, LimitValue, RegPulseKind, ResetDef
    }
};

use super::{
    casing::{Casing::{Snake, Title}, ToCasing},
    gen_common::{GeneratorBaseSetting, RifList}
};

pub struct GeneratorSv {
    base_settings: GeneratorBaseSetting,
    txt: String,
    stash: [String; 2],
    names: Vec<String>,
}

impl GeneratorSv {

    pub fn new(args: GeneratorBaseSetting) -> Self {
        GeneratorSv {
            base_settings: args,
            txt: String::with_capacity(10000),
            stash: [String::with_capacity(1000), String::with_capacity(1000)],
            names: Vec::new()
        }
    }

    fn write(&mut self, string: &str) {
        self.txt.push_str(string);
    }

    fn push_stash(&mut self, idx: usize, string: &str) {
        self.stash[idx].push_str(string);
    }

    fn pop_stash(&mut self, idx: usize) {
        self.txt.push_str(&self.stash[idx]);
        self.stash[idx].clear();
    }

    fn stash_is_empty(&self, idx: usize) -> bool {
        self.stash[idx].is_empty()
    }

    fn save(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path : PathBuf = [
            self.base_settings.path.clone(),
            filename.into()
        ].iter().collect();
        std::fs::write(path, self.txt.as_bytes())?;
        self.txt.clear();
        Ok(())
    }

    //-----------------------------

    pub fn gen(&mut self, obj: &Comp) -> Result<(), Box<dyn std::error::Error>> {
        // Create output directory if it does not exist
        create_dir_all(self.base_settings.path.clone())?;
        // Call relevant generator (Rif or Rifmux)
        match obj {
            Comp::Rif(rif) => {
                self.gen_pkg(rif)?;
                self.gen_rif(rif)?;
            }
            Comp::Rifmux(rifmux) => {
                self.gen_rifmux_pkg(rifmux)?;
                self.gen_rifmux(rifmux)?;
                // Generate include file
                if !self.base_settings.gen_inc.is_empty() {
                    let rif_list = RifList::new(rifmux);
                    for rif in rif_list.iter() {
                        if !self.base_settings.gen_inc.contains(&rif.inst_name) && self.base_settings.gen_inc.first()!=Some(&"*".to_owned()) {
                            continue;
                        }
                        self.gen_pkg(rif)?;
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

    //-----------------------------------------------------------------------------
    // RIF Package: enum & structure definition
    //-----------------------------------------------------------------------------

    fn gen_pkg(&mut self, rif: &RifInst) -> Result<(), Box<dyn std::error::Error>> {
        let rif_name = rif.name(true).to_casing(Snake); // TODO: handle prefixing
        // Add header : TODO: configurable header
        self.write("// File generated automatically by rifgen: DO NOT EDIT.\n\n");
        //
        self.write(&format!("package {rif_name}_pkg;\n\n"));
        // Localparams
        let rif_name_uc = rif.type_name.to_uppercase();
        self.write(&format!("   localparam int C_{rif_name_uc}_ADDR_W = {:2};\n", rif.addr_width));
        self.write(&format!("   localparam int C_{rif_name_uc}_DATA_W = {:2};\n", rif.data_width));
        for (k, &v) in rif.params.items() {
            let type_name = match v {
                0 | 1 => "bit",
                _ => "int",
            };
            self.write(&format!(
                "   localparam {type_name} C_{rif_name_uc}_{} = {v};\n",
                k.to_uppercase()
            ));
        }

        // Enums
        self.write("\n");
        let nb_enum = rif.enum_defs.iter().filter(|e| e.is_local_type()).count();
        if nb_enum > 0 {
            self.write("   // Enums\n");
            for enum_def in rif.enum_defs.iter().filter(|e| e.is_local_type()) {
                let msb = usize::BITS - (enum_def.len()-1).leading_zeros() - 1;
                self.write("   typedef enum logic ");
                if msb > 0 {
                    self.write(&format!("[{msb}:0] "));
                }
                self.write("{\n");
                for (i, v) in enum_def.iter().enumerate() {
                    self.write(&format!("      {} = {}", v.name, v.value));
                    self.write(if i < enum_def.len() - 1 {","} else {" "});
                    self.write(" // ");
                    self.write(v.description.get_short());
                    self.write("\n");
                }
                self.write("   } ");
                self.write(&enum_def.name);
                self.write(";\n\n");
            }
        }

        // Create two structures (hardware/software) per register type
        for hw_reg in rif.reg_impl_defs.values().filter(|r| r.pkg.is_none()) {
            // Check if any fields is an array to control if structure should be packed or not
            let mut hw_has_array = false;
            let mut sw_has_array = false;
            self.names.clear();
            // Iterate over all register fields to add them in the structs
            for f in hw_reg.fields.iter() {
                // Create signal declaration
                let mut t = String::with_capacity(32);
                let signess = if f.signed { "signed " } else { "" };
                let width = if f.sw_kind.is_password() {1} else {f.width};
                let name = f.name.to_casing(Snake);
                let Some(ctrl) = hw_reg.regs_ctrl.get(f.ctrl_idx) else {
                    return Err(format!("Field {}.{name} points to ctrl {} but max is {}",hw_reg.name, f.ctrl_idx, hw_reg.regs_ctrl.len()).into())
                };
                t.push_str("      ");
                match &f.enum_kind {
                    EnumKind::Type(n) => t.push_str(&format!("{} ",n)),
                    _ => {
                        t.push_str("logic ");
                        if f.signed {
                            t.push_str(signess);
                        }
                        if width > 1 {
                            t.push_str(&format!("[{}:0] ", width-1));
                        }
                    }
                }
                t.push_str(&name);
                // Suffix password by _locked
                if f.sw_kind.is_password() {
                    t.push_str("_locked");
                }
                if f.array > 0 {
                    t.push_str(&format!("[{}]", f.array));
                }
                t.push(';');
                if !f.description.is_empty() {
                    t.push_str(&format!(" // {}", f.description.get_short()));
                }
                t.push('\n');
                // Add field to SW structure writable by firmware or readable by hardware
                if (!f.is_local() || ctrl.external.is_rw()) && (f.is_sw_write() || f.is_constant() || f.is_counter() || f.hw_acc.is_readable()) {
                    self.push_stash(1, &t);
                    if f.array > 0 {
                        sw_has_array = true;
                    }
                }
                // Add field to HW structure if written by hardware
                if f.has_hw_value() || ctrl.external.is_rw() {
                    self.push_stash(0,&t);
                    if f.array > 0 {
                        hw_has_array = true;
                    }
                }
                // Add special fields
                for kind in f.hw_kind.iter() {
                    // Write modifiers: Write Enable, clr/set/tgl
                    if kind.has_write_mod() {
                        self.add_special_field(kind, &hw_reg.name, &name)?;
                    }
                    // Counter need multiple extra fields
                    else if let FieldHwKind::Counter(info) = kind {
                        if info.clr {
                            self.push_stash(0, &format!("      logic {0}_clr; // Clear counter {0}\n",name));
                        }
                        if info.kind == CounterKind::Up || info.kind == CounterKind::UpDown {
                            self.push_stash(0, &format!("      logic {0}_incr_en; // Increment counter {0}\n", name));
                        }
                        if info.kind == CounterKind::Down || info.kind == CounterKind::UpDown {
                            self.push_stash(0, &format!("      logic {0}_decr_en; // Decrement counter {0}\n", name));
                        }
                        if info.event || info.sat {
                            self.push_stash(1, &format!("      logic {0}_event; // Pulse high when {0} wrap/saturate\n", name));
                        }
                        if info.incr_val > 1 {
                            self.push_stash(0, &format!("      logic {signess}[{msb}:0] {n}_incr_val; // Increment value for counter {n}\n",msb=info.incr_val-1, n=name));
                        }
                        if info.decr_val > 1 {
                            self.push_stash(0, &format!("      logic {signess}[{msb}:0] {n}_decr_val; // Increment value for counter {n}\n",msb=info.incr_val-1, n=name));
                        }
                    }
                }
                if let FieldSwKind::Password(info) = &f.sw_kind {
                    if info.has_hold() {
                        self.push_stash(1, &format!("      logic {name}_hold; // High when {name}_locked is not changed by register access\n"));
                    }
                }
                // Clear
                if let Some(clr_sig) = &f.clear {
                    let clr_name = if clr_sig.is_empty() {format!("this.{name}_clr")} else {clr_sig.to_owned()};
                    let kind = FieldHwKind::Clear(Some(clr_name));
                    self.add_special_field(&kind, &hw_reg.name, &name)?;
                }
                // Lock signal from hardware
                if let Some(lock) = f.lock.local_name() {
                    if !lock.is_empty() && !self.names.iter().rev().any(|n| n==lock) {
                        self.names.push(lock.to_owned());
                        self.push_stash(0, &format!("      logic {lock}; // High to lock some field write access\n"));
                    }
                }
            }
            // Add fields for register pulse and external access
            let is_multi_pulse = hw_reg.is_multi_pulse();
            for ctrl in hw_reg.regs_ctrl.iter() {
                let (sep,name) = if is_multi_pulse {("_",&*ctrl.name)} else {("","")};
                for pulse in ctrl.pulse.iter() {
                    match pulse {
                        RegPulseKind::Write(_)  => self.push_stash(1, &format!("      logic p_{name}{sep}write; // Pulse high when register {} is written\n", ctrl.name)),
                        RegPulseKind::Read(_)   => self.push_stash(1, &format!("      logic p_{name}{sep}read; // Pulse high when register {} is read\n", ctrl.name)),
                        RegPulseKind::Access(_) => self.push_stash(1, &format!("      logic p_{name}{sep}acc; // Pulse high when register {} is accessed\n", ctrl.name)),
                    }
                }
                if ctrl.external != ExternalKind::None {
                    self.push_stash(0, &format!("      logic ext_{name}{sep}done; // Pulse high when read/write operation on register {} is complete\n", ctrl.name));
                    if matches!(ctrl.external, ExternalKind::ReadWrite | ExternalKind::Write) {
                        self.push_stash(1, &format!("      logic ext_{name}{sep}write; // Pulse high to start a write operation on register {}\n", ctrl.name));
                    }
                    if matches!(ctrl.external, ExternalKind::ReadWrite | ExternalKind::Read) {
                        self.push_stash(1, &format!("      logic ext_{name}{sep}read; // Pulse high to start a read operation on register {}\n", ctrl.name));
                    }
                }
            }
            let reg_name = hw_reg.name.to_casing(Snake);
            if !self.stash_is_empty(1) {
                let packed = if sw_has_array { "" } else { "packed " };
                self.write(&format!("   typedef struct {packed}{{\n"));
                self.pop_stash(1);
                self.write(&format!("   }} t_{reg_name}_sw;\n\n"));
            }
            if !self.stash_is_empty(0) {
                let packed = if hw_has_array { "" } else { "packed " };
                self.write(&format!("   typedef struct {packed}{{\n"));
                self.pop_stash(0);
                self.write(&format!("   }} t_{reg_name}_hw;\n\n"));
            }
        }

        self.write(&format!("endpackage : {rif_name}_pkg\n"));

        // Write file
        self.save(&format!("{rif_name}_pkg.sv"))?;
        Ok(())
    }

    fn add_special_field(&mut self, kind: &FieldHwKind, regname: &str, fieldname: &str) -> Result<(), String> {
        let name = if let Some(path) = kind.get_signal() {
            let mut parts = path.split('.');
            match (parts.next(),parts.next()) {
                (Some(f),None) => f.to_owned(),
                (Some(r),Some(f)) if r == regname || r == "this" || r == "self" => f.to_owned(),
                _ => "".to_owned()
            }
        } else {
            format!("{}{}", fieldname, kind.get_suffix())
        };
        //
        if !name.is_empty() && !self.names.iter().rev().any(|n| n==&name) {
            self.push_stash(0, &format!("      logic {name}; // {}\n", kind.get_comment(fieldname)));
            self.names.push(name);
        }
        Ok(())
    }


    //-----------------------------------------------------------------------------
    // RIF implementation: Address decoding, registers , ...
    //-----------------------------------------------------------------------------

    fn gen_rif(&mut self, rif: &RifInst) -> Result<(), Box<dyn std::error::Error>> {

        let addr_shift = (rif.data_width as f32).log2().ceil() as u8 - 3; // Min data width is 8 bits
        // Header (TODO: support external template)
        self.write("// File generated automatically: DO NOT EDIT.\n\n");
        let rif_name = rif.name(false).to_casing(Snake);
        let rif_pkg_name = rif.name(true).to_casing(Snake);
        self.write(&format!("module {rif_name}"));
        // Add parameters
        //
        self.write(" (\n");
        // Clocks/Reset/Clear
        let mut list_clocking = HashSet::with_capacity(2);
        self.add_clocking_port(&rif.sw_clocking, &mut list_clocking, false);
        for hw_clk in rif.hw_clocking.iter() {
            self.add_clocking_port(hw_clk, &mut list_clocking, true);
        }
        // Clock enables
        // println!("{:#?}", rif.ports);
        for clk_en in rif.ports.clk_ens.iter() {
            self.write_port(clk_en, None, 0, 0, false, false);
        }
        // Control signals
        if !rif.ports.ctrls.is_empty() {
            self.write("   // Input signals\n");
            for ctrl in rif.ports.ctrls.iter() {
                self.write_port(ctrl, None, 0, 0, false, false);
            }
        }
        // Collect external pages
        let mut ext_pages = Vec::new();
        for page in rif.pages.iter() {
            if let Some(width) = &page.external {
                ext_pages.push((page.name.to_lowercase(),page.addr,width));
                continue;
            }
        }

        // Input
        let mut interrupts = Vec::new();
        self.write("   // Input registers\n");
        for (group_name, hw_reg) in rif.hw_regs.items() {
            let hw_reg_def = rif.get_hw_reg(&hw_reg.group);
            let pkg_name = if let Some(pkg) = &hw_reg_def.pkg {pkg} else {&rif_pkg_name};
            let pkg_name = pkg_name.to_casing(Snake);
            let group_name = group_name.to_casing(Snake);
            let group_type = hw_reg.group.to_casing(Snake);
            let dim = if hw_reg.dim > 0 {format!("[{}]", hw_reg.dim)} else {"".to_owned()};
            let desc = hw_reg_def.description.get_short();
            if hw_reg.port.is_in() {
                self.write(&format!("   input var {pkg_name}_pkg::t_{group_type}_hw {group_name}{dim}, // {desc}\n"));
            }
            // Save the output register in the stash to be properly separated
            // println!("Reg {group_name} : def={:?}, inst={:?}", hw_reg_def.port, hw_reg.port);
            if hw_reg.port.is_out() {
                let kind = if hw_reg.intr_derived {"hw"} else {"sw"};
                self.push_stash(0,&format!("   output var {pkg_name}_pkg::t_{group_type}_{kind} rif_{group_name}{dim}, // {desc}\n"));
            }
            // Save interrupts in a Vec for later
            if !hw_reg_def.interrupt.is_empty() && !hw_reg.intr_derived {
                interrupts.push(group_name.clone());
                for info in hw_reg_def.interrupt.iter().skip(1) {
                    interrupts.push(format!("{group_name}_{}", info.name));
                }
            }
        }

        // Output registers
        self.write("   // Output registers\n");
        self.pop_stash(0);

        // Interrupt lines
        for irq in interrupts {
            self.write(&format!("   output var logic rif_{0}_irq, // High when one interrupt field of {0} is asserted\n",irq));
        }

        // Add control to external pages
        for (name,_, _) in ext_pages.iter() {
            self.write(&format!(
                "   rif_if.ctrl if_page_{0}, // Interface to access register from page {0}\n",
                name
            ));
        }

        // Add Main Control interface
        self.add_intf(&rif.interface, rif.addr_width, rif.data_width);
        self.write(");\n\n");

        //----------------------
        // Signals declaration
        self.write("/*------------------------------------------------------------------------------\n",);
        self.write("--  Signals declaration\n");
        self.write("------------------------------------------------------------------------------*/\n",);
        self.write(&format!("   logic [{}:0] rif_addr_l;\n", rif.addr_width - 1 - addr_shift));
        self.write(&format!("   logic [{}:0] rif_read_data_l;\n", rif.data_width - 1));
        self.write("   logic rif_err_addr_l, rif_err_access_l, rif_done_next;\n\n");

        // Declare local clock enable
        self.names.clear();
        for hw_clk in rif.hw_clocking.iter() {
            if !hw_clk.en.is_empty() && !self.names.contains(&hw_clk.en){
                self.write(&format!("   logic {}_l;\n",hw_clk.en));
                self.names.push(hw_clk.en.to_owned());
            }
        }
        // Declare Decode pulse / readback value per register
        for page in rif.pages.iter().filter(|p| p.external.is_none()) {
            for reg in page.regs.iter() {
                let name = reg.name().to_casing(Snake);
                self.write(&format!("   logic {name}__decode;\n"));
                self.write(&format!("   logic [{}:0] {name}__read_data;\n", rif.data_width - 1));
            }
        }
        self.write("\n");
        // Declare local signal per register group
        for (inst_name, hw_reg) in rif.hw_regs.items().filter(|(_,r)| !r.intr_derived) {
            let group_name = inst_name.to_casing(Snake);
            let hw_reg_def = rif.get_hw_reg(&hw_reg.group);
            let reg_dim = hw_reg.dim;
            let group_type = &hw_reg.group;
            let pkg_name = if let Some(pkg) = &hw_reg_def.pkg {pkg} else {&rif_pkg_name};
            let dim = if reg_dim > 0 {format!("[{reg_dim}]")} else {"".to_owned()};
            // Local register
            if hw_reg_def.is_local() {
                self.write(&format!("   {pkg_name}_pkg::t_{group_type}_sw rif_{group_name}{dim};\n"));
            }
            for idx_u16 in 0..reg_dim.max(1) {
                let idx = if reg_dim > 0 {format!("{idx_u16}")} else {"".to_owned()};
                // Interrupt register
                if hw_reg_def.is_interrupt() {
                    for intr_info in hw_reg_def.interrupt.iter() {
                        let name = if intr_info.name.is_empty() {&group_name} else {&intr_info.name};
                        // println!("Interrupt {}: {:?}", name, hw_reg_def.port);
                        if !hw_reg.port.is_out() {
                            self.write(&format!("   {pkg_name}_pkg::t_{group_type}_sw rif_{name}{idx};\n"));
                        }
                        self.write(&format!("   {pkg_name}_pkg::t_{group_type}_hw {name}{idx}_l;\n"));
                        // Add delay register if trigger works on edges
                        if intr_info.edge_trigger() {
                            self.write(&format!("   {pkg_name}_pkg::t_{group_type}_hw {name}{idx}_d1;\n"));
                        }
                        // Add optional enable/mask register
                        if intr_info.enable.is_some() {
                            let n = format!("{inst_name}_en");
                            if let Some(hw_reg_en) = rif.hw_regs.get(&n) {
                                if !hw_reg_en.port.is_out() {
                                    self.write(&format!("   {pkg_name}_pkg::t_{group_type}_hw rif_{name}{idx}_en;\n"));
                                }
                            }
                        }
                        if intr_info.mask.is_some() {
                            let n = format!("{inst_name}_mask");
                            if let Some(hw_reg_mask) = rif.hw_regs.get(&n) {
                                if !hw_reg_mask.port.is_out() {
                                    self.write(&format!("   {pkg_name}_pkg::t_{group_type}_hw rif_{name}{idx}_mask;\n"));
                                }
                            }
                        }
                        // Internal pending signal is always present (used to generate the irq output)
                        self.write(&format!("   {pkg_name}_pkg::t_{group_type}_hw rif_{name}{idx}_pending;\n"));
                        self.write(&format!("   logic clk_en_intr_{name}{idx};\n"));
                        // Add next signal for each field
                        for f in hw_reg_def.fields.iter() {
                            let f_name = f.name.to_casing(Snake);
                            let range =
                                if f.width > 1 {format!(" [{}:0]", f.width - 1)}
                                else {"".to_owned()};
                            self.write(&format!("   logic{range} {name}{idx}_{f_name}__next;\n"));
                            if intr_info.enable.is_some() {
                                self.write(&format!("   logic{range} {name}{idx}_en_{f_name}__next;\n"));
                            }
                            if intr_info.mask.is_some() {
                                self.write(&format!("   logic{range} {name}{idx}_mask_{f_name}__next;\n"));
                            }
                        }
                    }
                    continue;
                }
                // Field combinatorial next value
                for f in hw_reg_def.fields.iter() {
                    let f_name = f.name.to_casing(Snake);
                    // Add signal to handle out-of-limit check
                    if f.limit.value != LimitValue::None {
                        self.write(&format!("   logic {group_name}{idx}_{f_name}__check;\n"));
                    }
                    // Skip external field
                    let Some(ctrl) = hw_reg_def.regs_ctrl.get(f.ctrl_idx) else {
                        return Err(format!("Field {}.{} points to ctrl {} but max is {}", hw_reg_def.name, f.name, f.ctrl_idx, hw_reg_def.regs_ctrl.len()).into())
                    };
                    if ctrl.is_external() {
                        continue;
                    }
                    // Skip disabled field
                    // if f.disabled {
                    //     continue;
                    // }
                    // No next for combinatorial pulse or read only field from hardware with no register
                    if f.sw_kind.is_pulse_comb() || (f.sw_kind==FieldSwKind::ReadOnly && !f.has_write_mod() && !f.is_counter()) {
                        continue;
                    }
                    let tn = match &f.enum_kind {
                        EnumKind::Type(t) => {
                            if !t.contains("::") {
                                format!("{pkg_name}_pkg::{t}")
                            } else {
                                t.to_owned()
                            }
                        }
                        _ => {
                            let signed = if f.signed { "signed " } else { "" };
                            let width =
                                if f.sw_kind.is_password() {2}
                                else if f.is_counter() {f.width+1}
                                else {f.width};
                            let range =
                                if width > 1 {format!("[{}:0]", width - 1)}
                                else {"".to_owned()};
                            format!("logic {signed}{range}")
                        }
                    };
                    if f.array > 0 {
                        for i in 0..f.array {
                            self.write(&format!("   {tn} {group_name}{idx}_{f_name}{i}__next;\n"));
                        }
                    } else {
                        self.write(&format!("   {tn} {group_name}{idx}_{f_name}__next;\n"));
                    }
                    // Add register to store local value (when register is not visible at the output)
                    if f.is_local() {
                        self.write(&format!("   {tn} {group_name}{idx}_{f_name}__reg;\n"));
                    }
                }
            }
        }

        // Add interface bridge when not default
        self.add_intf_bridge(&rif.interface, rif.addr_width, rif.data_width, &rif.sw_clocking.clk, &rif.sw_clocking.rst.name);

        // Interface handline
        self.write("\n/*------------------------------------------------------------------------------\n",);
        self.write("--  Interface handling\n");
        self.write("------------------------------------------------------------------------------*/\n",);
        // TODO: handle option pipe==0
        let signals: Vec<SignalInfo> = vec![
            SignalInfo::new("if_rif.err_addr"  , 1, "1'b0", "rif_err_addr_l   & if_rif.en"),
            SignalInfo::new("if_rif.err_access", 1, "1'b0", "rif_err_access_l & if_rif.en"),
            SignalInfo::new("if_rif.done", 1, "1'b0", "rif_done_next"),
            SignalInfo::new_with_en("if_rif.rd_data", rif.data_width, &format!("{}'b0", rif.data_width), "rif_read_data_l", "rif_done_next & if_rif.rd_wrn"),
        ];
        self.gen_process(
            &rif.sw_clocking.clk,
            &rif.sw_clocking.rst,
            "proc_if_rif",
            &signals,
        );
        self.write("   assign if_rif.done_next       = rif_done_next   ;\n");
        self.write("   assign if_rif.err_addr_next   = rif_err_addr_l  ;\n");
        self.write("   assign if_rif.err_access_next = rif_err_access_l;\n\n");
        self.write(&format!(
            "   assign rif_addr_l = if_rif.addr[{}:{}];\n\n",
            rif.addr_width - 1,
            addr_shift
        ));

        // Hardware clock enable: add register access to ensure field can be modify  by firmware
        self.names.clear();
        for hw_clk in rif.hw_clocking.iter() {
            if !hw_clk.en.is_empty() && !self.names.contains(&hw_clk.en){
                self.write(&format!("   assign {0}_l = {0} || if_rif.en;\n",hw_clk.en));
                self.names.push(hw_clk.en.to_owned());
            }
        }

        // Decode process
        self.write("   always_comb begin : proc_decode\n");
        self.write(&format!("      rif_read_data_l = {}'b0;\n", rif.data_width));
        if ext_pages.is_empty() {
            self.write("      rif_done_next    = if_rif.en;\n");
            self.write("      rif_err_addr_l   = 1'b1;\n");
            self.write("      rif_err_access_l = 1'b1;\n");
        } else {
            // let page_en = ext_pages.iter().map(|n| format!("if_page_{}.en", n.to_lowercase())).collect();
            let page_en: Vec<String> = ext_pages
                .iter()
                .map(|(n,_,_)| format!("if_page_{}.en", n))
                .collect();
            let page_en = page_en.join(" | ");
            self.write(&format!("      rif_err_addr_l   = ~({});\n", page_en));
            self.write(&format!("      rif_err_access_l = ~({});\n", page_en));
            self.write(&format!("      rif_done_next = (if_rif.en & ~({}))\n", page_en));
            for (name,_,_) in ext_pages.iter() {
                self.write(&format!(
                    "\n         | (if_page_{name}.en & if_page_{name}.done)",
                ));
            }
            self.write(";\n");
        }
        for page in rif.pages.iter().filter(|p| p.external.is_none()) {
            for reg in page.regs.iter() {
                self.write(&format!("      {}__decode = 1'b0;\n", reg.name().to_casing(Snake)));
            }
        }
        self.write("      case(rif_addr_l)\n");
        for page in rif.pages.iter().filter(|p| p.external.is_none()) {
            for reg in page.regs.iter() {
                let name_flat = reg.name().to_casing(Snake);
                let group_name = reg.group_name.to_casing(Snake);
                self.write(&format!(
                    "         {}'d{} : begin\n",
                    rif.addr_width - addr_shift,
                    (reg.addr + page.addr) >> addr_shift
                ));
                self.write(&format!("            {name_flat}__decode = "));
                let field_limit: Vec<(String, String)> = reg
                    .fields
                    .iter()
                    .filter(|field| field.limit.value != LimitValue::None)
                    .map(|field| (field.name.to_owned(), field.limit.bypass.to_owned()))
                    .collect();
                if !field_limit.is_empty() {
                    self.write("if_rif.rd_wrn || (");
                    for (i, fl) in field_limit.iter().enumerate() {
                        if i != 0 {
                            self.write(" && ");
                        }
                        if fl.1.is_empty() {
                            self.write(&format!("{group_name}_{}__check", fl.0));
                        } else {
                            self.write(&format!("({group_name}_{}__check || {})", fl.0, fl.1));
                        }
                    }
                    self.write(");\n");
                } else {
                    self.write("1'b1;\n");
                }
                self.write(&format!("            rif_read_data_l   = {name_flat}__read_data;\n"));
                self.write("            rif_err_addr_l    = 1'b0;\n");
                // Access error when writing a read-only field, reading a write only field,
                //  or writing one field outside its set value (when limits are defined)
                self.write("            rif_err_access_l  = ");
                match reg.sw_access {
                    Access::RO => self.write("~if_rif.rd_wrn;\n"),
                    Access::WO => self.write("if_rif.rd_wrn;\n"),
                    Access::NA => self.write("1'b1;\n"),
                    Access::RW => {
                        if field_limit.is_empty() {
                            self.write("1'b0;\n");
                        } else {
                            self.write(&format!("~{name_flat}__decode;\n"));
                        }
                    },
                }
                // Handle external register
                if reg.external!=ExternalKind::None {
                    let hw_reg_def = rif.get_hw_reg(&reg.group_type);
                    let idx = if let ArrayIdx::Inst(idx,_)= reg.array {format!("[{idx}]")} else {"".to_owned()};
                    // let dim = if *reg_dim > 0 {format!("[{reg_dim}]")} else {"".to_owned()};
                    self.write(&format!("            rif_done_next = {group_name}{idx}.ext_"));
                    if hw_reg_def.is_multi_pulse() {
                        self.write(&name_flat);
                        self.write("_");
                    }
                    self.write("done;\n");
                }
                self.write("         end\n");
            }
        }
        // Handle external pages
        if !ext_pages.is_empty() {
            self.write("      default: begin\n");
            for (i,(name,_,_)) in ext_pages.iter().enumerate() {
                let name = name.to_casing(Snake);
                self.write(&format!("            {}if(if_page_{}.done) begin\n",if i!=0 {"else"} else {""},name));
                self.write(&format!("               rif_read_data_l  = if_page_{name}.rd_data;\n"));
                self.write(&format!("               rif_err_addr_l   = if_page_{name}.err_addr;\n"));
                self.write(&format!("               rif_err_access_l = if_page_{name}.err_access;\n"));
                self.write("            end\n");
            }
            self.write("      end\n");
        }

        self.write("      endcase\n");
        self.write("   end\n\n");

        // Control the external page interface
        for (name,addr,&width) in ext_pages.iter() {
            let name = name.to_casing(Snake);
            self.write(&format!("   assign if_page_{name}.addr    = if_rif.addr   ;\n"));
            self.write(&format!("   assign if_page_{name}.rd_wrn  = if_rif.rd_wrn ;\n"));
            self.write(&format!("   assign if_page_{name}.wr_data = if_rif.wr_data;\n"));
            // self.write(&format!("   assign if_page_{}.wr_mask = if_rif.wr_mask;\n",page));
            self.write(&format!("   assign if_page_{name}.en      = if_rif.en && if_rif.addr[{}:{}]=={};\n",
                rif.addr_width-1, width, addr >> width));
        }

        // Register Process
        self.write("/*------------------------------------------------------------------------------\n");
        self.write("--  Registers\n");
        self.write("------------------------------------------------------------------------------*/\n\n");

        let mut group_done : HashSet<String> = HashSet::with_capacity(rif.hw_regs.len());
        for page in rif.pages.iter().filter(|p| p.external.is_none()) {
            for reg in page.regs.iter() {
                let reg_impl = rif.get_hw_reg(&reg.group_type);
                // Save a few string to be reused
                let reg_name  = reg.name().to_casing(Snake); // Register Name with index apped after
                let group_name = reg.group_name().to_casing(Snake); // Group name without index
                let reg_name_i   = reg.name_i().to_casing(Snake); // Register name with optional index in bracket
                let group_name_i = reg.group_name_i().to_casing(Snake); // Group name with optional index in bracket
                let reg_idx    = if let ArrayIdx::Inst(idx,_) = reg.array {format!("{idx}")} else {"".to_owned()};
                let reg_idxb   = if !reg_idx.is_empty() {format!("[{reg_idx}]")} else {"".to_owned()};
                let intr_suffix = reg.intr_info.0.get_suffix();
                self.write(&format!("   // Register {reg_name_i}\n"));
                // Assign field
                for field in reg.fields.iter() {
                    let field_impl = reg_impl.get_field(&field.name)?;
                    // Handle partial field
                    let partial_range = if let Some(partial_pos) = field.partial.0 {
                        if field.width > 1 {
                            format!("[{}:{}]",partial_pos + field.width as u16 - 1, partial_pos)
                        } else {
                            format!("[{}]",partial_pos)
                        }
                    } else {
                        "".to_string()
                    };

                    let field_name = field.name().to_casing(Snake);
                    let field_name_flat = field.name_flat().to_casing(Snake);
                    // Local field: for partial field ensure the current one is also local
                    let field_path = if field_impl.is_local() && field.has_write_mod() && !reg.is_external() {
                        format!("{group_name}{intr_suffix}{reg_idx}_{field_name_flat}__reg{partial_range}")
                    } else {
                        format!("rif_{group_name}{intr_suffix}{reg_idxb}.{field_name}{partial_range}")
                    };
                    let reg_field_name = format!("{group_name}{intr_suffix}{reg_idx}_{field_name_flat}");

                    let reset_str = Self::field_reset_str(field, false, &rif_pkg_name, &reg.reg_type);

                    // Disabled field ? simply assign to its reset value
                    if field.is_disabled() && (field.sw_kind==FieldSwKind::ReadWrite || field.sw_kind==FieldSwKind::WriteOnly) {
                        self.write(&format!("   assign {field_path} = {reset_str}; // Disabled\n"));
                        continue;
                    }

                    // Constant field
                    if field_impl.is_constant() {
                        self.write(&format!("   assign {field_path} = {reset_str}; \n"));
                        continue;
                    }

                    // Construct the field value from the bus with bit selection
                    // For non partial field, add proper casting (signed/enum)
                    let mut field_val = "if_rif.wr_data".to_string();
                    if field.width > 1 {
                        field_val.push_str(&format!("[{}:{}]",field.msb(), field.lsb));
                    } else {
                        field_val.push_str(&format!("[{}]",field.lsb));
                    };
                    if !field_impl.is_partial {
                        field_val = Self::add_cast(&field_val, field, &rif_pkg_name, &reg.group_type);
                    }

                    // Add logic for field with limit
                    if field.has_limit() {
                        self.write(&format!("   assign {reg_field_name}__check = "));
                        match &field.limit.value {
                            LimitValue::Min(min) => {
                                let min = Self::value_to_str(min.to_u128(field.width), field.width.into(), field.is_signed(), false);
                                self.write(&format!("{} >= {min}",field_val));
                            }
                            LimitValue::Max(max) => {
                                let max = Self::value_to_str(max.to_u128(field.width), field.width.into(), field.is_signed(), false);
                                self.write(&format!("{} <= {max}",field_val));
                            }
                            LimitValue::MinMax(min, max) => {
                                let min = Self::value_to_str(min.to_u128(field.width), field.width.into(), field.is_signed(), false);
                                let max = Self::value_to_str(max.to_u128(field.width), field.width.into(), field.is_signed(), false);
                                self.write(&format!("{0} >= {min} && {0} <= {max} ",field_val));
                            }
                            LimitValue::List(l) => {
                                for (i,e) in l.iter().enumerate() {
                                    let e_val = Self::value_to_str(e.to_u128(field.width), field.width.into(), field.is_signed(), false);
                                    self.write(&format!("{field_val} == {e_val}"));
                                    if i < l.len()-1 {
                                        self.write(" || ");
                                    }
                                }
                            },
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
                                for (i,e) in enum_def.iter().enumerate() {
                                    self.write(&format!("{} == {:?}",field_val, e.value));
                                    if i < enum_def.len()-1 {
                                        self.write(" || ");
                                    }
                                }
                            },
                            // No limit -> nothing to do
                            LimitValue::None => {},
                        }
                        self.write(";\n");
                    }

                    // For external register combinatorial assign from the interface bus
                    // Also add logic for enum field with limit
                    if reg.is_external() && field.is_sw_write() {
                        self.write(&format!("   assign {} = {};\n", field_path, field_val));
                        continue;
                    }

                    // Combinatorial pulse : direct assign
                    if field.sw_kind.is_pulse_comb() {
                        self.write(&format!("   assign {field_path} = "));
                        self.write(&format!("{reg_name}__decode & if_rif.en & ~if_rif.rd_wrn ? "));
                        self.write(&format!("{field_val} : {}'b0;\n", field.width));
                        continue;
                    }

                    // Counter event (counter kind is exclusive so take fist one)
                    if let Some(FieldHwKind::Counter(info)) = field.hw_kind.first() {
                        if info.sat || info.event {
                            let msb = field.width-1;
                            self.write(&format!("   assign {}_event = ", field_path));
                            if field.is_sw_write() {
                                let pol = if field.sw_kind==FieldSwKind::ReadClr {"~"} else {""};
                                self.write(&format!("(~{reg_name}__decode | ~if_rif.en | {pol}if_rif.rd_wrn) & "));
                            }
                            self.write("(\n");
                            if field.is_signed() {
                                self.write(&format!("      {reg_field_name}__next[{}] ^ {reg_field_name}__next[{msb}]",
                                    field.width));
                            } else {
                                if info.incr_val > 0 {
                                    self.write(&format!("      (~{reg_field_name}__next[{msb}] & {field_path}[{msb}] & {field_path}_incr_en)"));
                                }
                                if info.decr_val > 0 {
                                    if info.incr_val > 0 {
                                        self.write(" |\n");
                                    }
                                    self.write(&format!("      ({reg_field_name}__next[{msb}] & ~{field_path}[{msb}] & {field_path}_decr_en)"));
                                }
                            }
                            self.write(");\n");
                        }
                    }

                    // Generate intermediate signal for interrupt
                    if reg.is_intr() {
                        let intr_info = reg_impl.intr_info(reg)?;
                        // Local signal where interrupt vector is and with the optional enable signals
                        self.write(&format!("   assign {0}_l.{1} = {0}.{1}", group_name, field_name));
                        if intr_info.enable.is_some() {
                            self.write(&format!(" & rif_{}_en.{}", group_name, field_name));
                        }
                        self.write(";\n");
                        // Next
                        self.write(&format!("   assign {reg_field_name}__next{partial_range} = "));
                        if let Some(FieldHwKind::Interrupt(intr_trig)) = field.hw_kind.first() {
                            match intr_trig {
                                InterruptTrigger::High    => self.write(&format!("{group_name}_l.{}", field_name)),
                                InterruptTrigger::Low     => self.write(&format!("~{group_name}_l.{}", field_name)),
                                InterruptTrigger::Rising  => self.write(&format!("({0}_l.{1} & ~{0}_d1.{1})", group_name, field_name)),
                                InterruptTrigger::Falling => self.write(&format!("(!{0}_l.{1} & ~{0}_d1.{1})", group_name, field_name)),
                                InterruptTrigger::Edge    => self.write(&format!("({0}_l.{1} != {0}_d1.{1})", group_name, field_name)),
                            }
                        }
                        self.write(&format!(" |\n      ({reg_name}__decode & if_rif.en & "));
                        match intr_info.clear {
                            InterruptClr::Read => self.write(&format!("if_rif.rd_wrn ? {}'b0", field.width)),
                            InterruptClr::Write0 => self.write(&format!("~if_rif.rd_wrn ? ({} & {})", field_val, field_path)),
                            InterruptClr::Write1 => self.write(&format!("~if_rif.rd_wrn ? (~{} & {})", field_val, field_path)),
                            InterruptClr::Hw => todo!(),
                        }
                        self.write(&format!(" : {});\n", field_path));
                        continue;
                    }

                    // Register derived from interrupt (i.e. enable/mask)
                    // Basic read/write register
                    if reg.is_intr_derived() && reg.intr_info.0 !=InterruptRegKind::Pending {
                        self.write(&format!("   assign {reg_field_name}__next{partial_range} = \n      "));
                        self.write(&format!("{reg_name}__decode & if_rif.en & ~if_rif.rd_wrn ? {field_val} :\n      "));
                        self.write(&format!("{field_path};\n"));
                        continue;
                    }

                    // Generate next value
                    if field.is_hw_write() || field.is_sw_write() {

                        // Generate __next signal
                        self.write(&format!("   assign {reg_field_name}__next{partial_range} = \n      "));

                        let mut cnt_info : Option<&CounterInfo> = None;
                        let idx = if let Some(partial_pos) = field.partial.0 {format!("_{}",partial_pos)} else {"".to_owned()};
                        // if reg.reg_name == "" {println!("{} : Hw={:?} Sw={:?}", field.name, field.hw_kind, field.sw_kind);}

                        // Handle hardware access
                        if field.is_hw_write() {
                            for kind in field.hw_kind.iter() {
                                let suffix = kind.get_suffix();
                                match kind {
                                    FieldHwKind::WriteEn(info) => {
                                        let sig = Self::get_signal_name(info, suffix, &reg.group_type, &group_name, &reg_idxb, &field_name, &idx);
                                        self.write(&format!("{sig} ? {group_name_i}.{field_name}{partial_range}"));
                                    },
                                    FieldHwKind::WriteEnL(info) => {
                                        let sig = Self::get_signal_name(info, suffix, &reg.group_type, &group_name, &reg_idxb, &field_name, &idx);
                                        self.write(&format!("~{sig} ? {group_name_i}.{field_name}{partial_range}"));
                                    },
                                    FieldHwKind::Set(info) => {
                                        let sig = Self::get_signal_name(info, suffix, &reg.group_type, &group_name, &reg_idxb, &field_name, &idx);
                                        self.write(&format!("{sig} ? "));
                                        if field.width == 1 {
                                            self.write("1'b1");
                                        } else {
                                            self.write(&format!("{field_path} | {group_name_i}.{field_name}{partial_range}"));
                                        }
                                    },
                                    FieldHwKind::Clear(info) => {
                                        let sig = Self::get_signal_name(info, suffix, &reg.group_type, &group_name, &reg_idxb, &field_name, &idx);
                                        self.write(&format!("{sig} ? "));
                                        if field.width == 1 {
                                            self.write("1'b0");
                                        } else {
                                            self.write(&format!("{field_path} & ~{group_name_i}.{field_name}{partial_range}"));
                                        }
                                    },
                                    FieldHwKind::Toggle(info) => {
                                        let sig = Self::get_signal_name(info, suffix, &reg.group_type, &group_name, &reg_idxb, &field_name, &idx);
                                        self.write(&format!("{sig} ? "));
                                        if field.width == 1 {
                                            self.write(&format!("~{field_path}"));
                                        } else {
                                            self.write(&format!("{field_path} ^ {group_name_i}.{field_name}{partial_range}"));
                                        }
                                    },
                                    // Counter : save the info for later implementation (counter has less prevalence than software access)
                                    FieldHwKind::Counter(info) => {
                                        cnt_info = Some(info);
                                    },
                                    // Nothing todo for other HwKind (already handled for interrupt)
                                    FieldHwKind::ReadOnly => {},
                                    FieldHwKind::Interrupt(_) => {},
                                }
                                if !matches!(kind, FieldHwKind::Counter(_)) {
                                    self.write(" :\n      ");
                                }
                            }
                        }

                        if field.is_sw_write() {
                            self.write(&format!("{reg_name}__decode & if_rif.en "));
                            // Handle Software access
                            match &field.sw_kind {
                                FieldSwKind::ReadWrite |
                                FieldSwKind::WriteOnly => self.write(&format!("& ~if_rif.rd_wrn ? {field_val}")),
                                FieldSwKind::ReadClr   => self.write(&format!("& if_rif.rd_wrn ? {}'b0", field.width)),
                                FieldSwKind::W1Clr => {
                                    self.write("& ~if_rif.rd_wrn ");
                                    if field.width == 1 {
                                        self.write(&format!("& {field_val} ? 1'b0"));
                                    } else {
                                        self.write(&format!("? {field_path} & ~{field_val}"));
                                    }
                                }
                                FieldSwKind::W0Clr => {
                                    self.write("& ~if_rif.rd_wrn ");
                                    if field.width == 1 {
                                        self.write(&format!("& ~{field_val} ? 1'b0"));
                                    } else {
                                        self.write(&format!("{field_path} & {field_val}"));
                                    }
                                }
                                FieldSwKind::W1Set |
                                FieldSwKind::W1Pulse(_,_) => {
                                    self.write("& ~if_rif.rd_wrn ? ");
                                    if field.width > 1 {
                                        self.write(&format!("{field_path} | "));
                                    }
                                    self.write(&field_val);
                                }
                                FieldSwKind::W1Tgl => {
                                    self.write("& ~if_rif.rd_wrn ? ");
                                    if field.width == 1 {
                                        self.write(&format!("~{field_path}", ));
                                    } else {
                                        self.write(&format!("{field_path} ^ {field_val}"));
                                    }
                                }
                                FieldSwKind::Password(info) => {
                                    self.write("& ~if_rif.rd_wrn");
                                    if info.protect || (info.once.is_some() && info.hold.is_some()) {
                                        self.write(&format!("& ({field_path}_hold | ~{field_path}_locked)"));
                                    }
                                    self.write("? (");
                                    if let Some(v) = &info.once {
                                        self.write(&format!("{}=={} ? 2'd0 : ", field_val, Self::value_to_str(v.to_u128(field.width), field.width.into(), false, true)));
                                    }
                                    if let Some(v) = &info.hold {
                                        self.write(&format!("{}=={} ? 2'd2 : ", field_val, Self::value_to_str(v.to_u128(field.width), field.width.into(), false, true)));
                                    }
                                    if info.protect {
                                        self.write(&format!("{field_val}!=0 ? 2'd3 : "));
                                    }
                                    self.write("2'd1)");
                                    // For once password, reset to 1 when writing on any other register
                                    if info.once.is_some() {
                                        self.write(" :\n      if_rif.en & ~if_rif.rd_wrn");
                                        if info.hold.is_some() {
                                            self.write(&format!(" & ~{field_path}_hold"));
                                        }
                                        self.write(" ? 2'd1");
                                    }
                                },
                                // Read Only case should be impossible due to the is_sw_write check earlier
                                FieldSwKind::ReadOnly => {},
                            }
                            self.write(" :\n      ");
                        }

                        // Handle Counter
                        if let Some(info) = cnt_info {
                            // println!("Counter {} : {:?}",field_name, info);
                            let hw_path = &field_path[4..];
                            if info.clr {
                                self.write(&format!("{hw_path}_clr ? {reset_str} :\n      "));
                            }
                            if info.is_up() {
                                self.write(&format!("{hw_path}_incr_en ? {field_path} + "));
                                if info.incr_val <= 1 {
                                    self.write(&Self::value_to_str(1, field.width.into(), field.is_signed(), false));
                                } else {
                                    self.write(&format!("{hw_path}_incr_val"));
                                }
                                self.write(" :\n      ");
                            }
                            if info.is_down() {
                                self.write(&format!("{hw_path}_decr_en ? {field_path} + "));
                                if info.decr_val <= 1 {
                                    self.write(&Self::value_to_str(1, field.width.into(), field.is_signed(), false));
                                } else {
                                    self.write(&format!("{hw_path}_decr_val"));
                                }
                                self.write(" :\n      ");
                            }
                        }

                        // Default next to current value
                        match &field.sw_kind {
                            FieldSwKind::W1Pulse(_,_) => self.write("1'b0;\n"),
                            FieldSwKind::Password(info) => {
                                if info.hold.is_some() {
                                    self.write(&format!("{{{field_path}_hold,"))
                                } else {
                                    self.write("{1'b0,");
                                }
                                self.write(&format!("{field_path}_locked}};\n"))
                            }
                            _ => self.write(&format!("{field_path};\n"))
                        }
                    }
                    // Handle case of partial field where one part is read-only
                    else if field_impl.has_write_mod() {
                        self.write(&format!("   assign {reg_field_name}__next{partial_range} = {}'b0; // unused\n", field.width));
                    }

                }
                // External register
                if reg.is_external() {
                    let mut sig_name = format!("rif_{group_name_i}.ext");
                    if reg_impl.regs_ctrl.len() > 1 {
                        sig_name.push_str(&format!("_{}",reg.reg_name));
                    }
                    if reg.sw_access.is_writable() {
                        self.write(&format!("   assign {sig_name}_write = {reg_name}__decode && if_rif.en && ~if_rif.rd_wrn;\n"));
                    }
                    if reg.sw_access.is_readable() {
                        self.write(&format!("   assign {sig_name}_read = {reg_name}__decode && if_rif.en && if_rif.rd_wrn;\n"));
                    }
                }
                // Sequential process
                else if reg.has_proc() {
                   // Get a default clock for the register
                    let hw_clk = &rif.hw_clocking.first().unwrap_or(&rif.sw_clocking);
                    let reg_clk =
                        if let Some(n) = &reg_impl.clk {n}
                        else if reg.sw_access.is_writable() && !reg.is_intr() {&rif.sw_clocking.clk}
                        else {&hw_clk.clk};
                    // println!("{} : clk={:?}, hw_access={:?} -> {reg_clk}", reg.reg_name, reg_impl.clk, reg.hw_access);
                    // Collect each field signal info in a hashmap indexed by a couple (clk/rst)
                    let mut signals: OrderDict<(String,String), Vec<SignalInfo> > = OrderDict::new();
                    for field in reg.fields.iter() {
                        // Get field implementation
                        let field_impl = reg_impl.get_field(&field.name)?;
                        let field_idxb = if field.array.dim() > 0 {format!("[{}]", field.array.idx())} else {"".to_owned()};
                        // Ignore disabled fields and partial fields after the first one
                        let partial_pos = field.partial.0.unwrap_or(0);
                        if field.is_disabled() || partial_pos!=0 || field.sw_kind.is_pulse_comb() {
                            continue;
                        }
                        // Combinatorial pulse and readonly field with no hardware
                        if field.sw_kind.is_pulse_comb() || (!field_impl.is_hw_write() && !field_impl.is_sw_write()) {
                            continue;
                        }
                        let field_name = field.name().to_casing(Snake);
                        let field_name_flat = field.name_flat().to_casing(Snake);
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
                        // if reg.reg_name=="" {println!("Field {} : Kind={:?} hw_write={} -> {f_rst} | reg reset={:?} | hw clocking={:?}", field.name, field.hw_kind, field.is_hw_write(), reg_impl.rst, hw_clk);}
                        // if field.name=="syncword_rx" {
                        //     println!("Field {} : Kind={:?}/{:?}/{:?} hw_acc={} local={}", field.name, field.hw_kind, field_impl.hw_kind, field_impl.sw_kind, field_impl.hw_acc, field_impl.is_local());
                        // }
                        // Name of the signal
                        let mut name = if field_impl.is_local() {
                            format!("{group_name}{intr_suffix}{reg_idx}_{field_name_flat}__reg")
                        } else {
                            format!("rif_{group_name}{intr_suffix}{reg_idxb}.{field_name}")
                        };
                        // Next value
                        let mut value = format!("{group_name}{intr_suffix}{reg_idx}_{field_name_flat}__next");
                        // Enable
                        let mut enable = if reg.is_intr() {
                            format!("clk_en_intr_{group_name_i}")
                        } else if let ClkEn::Signal(clk_en) = &field_impl.clk_en {
                            clk_en.clone()
                        } else if let ClkEn::Signal(clk_en) = &reg_impl.clk_en {
                            clk_en.clone()
                        } else if field.is_hw_write() {
                            hw_clk.en.clone()
                        } else {
                            rif.sw_clocking.en.clone()
                        };
                        if !enable.is_empty() && enable == hw_clk.en {
                            enable.push_str("_l");
                        }
                        // println!("Clock enable for Field {group_name}.{field_name} : field={:?}, reg={:?}, hw_write ? {}" , field_impl.clk_en, reg_impl.clk_en, field.is_hw_write());
                        if field_impl.lock.is_some() {
                            if !enable.is_empty() {
                                enable.push_str(" & ");
                            }
                            let lock_name = Self::get_signal_name(field_impl.lock.name(), "_lock", &reg.group_type,  &group_name, &reg_idxb, &field.name, &field_idxb);
                            enable.push_str(&format!("~{lock_name}"));
                        }
                        if let Some(FieldHwKind::Counter(cnt_info)) = field.hw_kind.first() {
                            if cnt_info.sat && cnt_info.incr_val <= 1 && cnt_info.decr_val <= 1 {
                                enable.push_str(&format!(" & ~rif_{group_name_i}.{}_event", field.name));
                            }
                            if field.array.dim() > 0 {todo!("Support field array of counters")}
                        }
                        // Clear
                        let clear = if reg_impl.clear.is_some() {
                            Self::get_signal_name(&reg_impl.clear, "reg_clr", &reg.group_type,  &group_name, &reg_idxb, "", "")
                        } else if field_impl.clear.is_some() {
                            Self::get_signal_name(&field_impl.clear, "_clr", &reg.group_type,  &group_name, &reg_idxb, &field.name, &field_idxb)
                        } else {
                            "".to_string()
                        };

                        // Signal Width: field width except for special fields
                        let width = if field.is_password() {1} else {field.width};

                        let reset = if field.is_password() {
                                "1'b1".to_owned()
                            } else if field.partial.0.is_some() {
                                // TODO: change
                                let rst_val = field_impl.get_reset(reg.group_idx);
                                Self::value_to_str(rst_val, field_impl.width, field_impl.signed, true)
                            } else {
                                Self::field_reset_str(field, false, &rif_pkg_name, &reg.reg_type)
                            };

                        // Handle Special cases
                        if field.is_password() {
                            name.push_str("_locked");
                            value.push_str("[0]");
                        }
                        else if let Some(FieldHwKind::Counter(cnt_info)) = field.hw_kind.first() {
                            if cnt_info.sat && (cnt_info.incr_val > 1 || cnt_info.decr_val > 1) {
                                let sat = if field.is_signed() {
                                    format!("$signed({{{0}[{1}],{{{2}{{~{0}[{1}]}}}}}})",value, field.width, field.width-1)
                                } else {
                                    format!("{{{0}{{~{1}[{0}]}}}}}}",field.width,value)
                                };
                                value = format!("rif_{group_name_i}.{}_event ? {sat} : {value}", field.name);
                            }
                        }

                        // Add the signal info the hashmap
                        let k = (f_clk.to_string(),f_rst.to_string());
                        let field_entry = signals.entry(&k);

                        field_entry.push(
                            SignalInfo::new_with_en_clr(&name, width, &reset, &value, &enable, &clear)
                        );

                        // For password protected or with both option once/hold, add another signal
                        if let FieldSwKind::Password(info) = &field.sw_kind {
                            if info.has_hold() {
                                let name = format!("rif_{group_name_i}.{}_hold", field.name);
                                field_entry.push(
                                    SignalInfo::new_with_en_clr(
                                        &name, width,
                                        "1'b0",
                                        &format!("{group_name}{reg_idx}_{field_name_flat}__next[1]"),
                                        &enable, &clear)
                                );
                            }
                        }
                        // For interrupt on edge, add delay version of the interrupt event
                        else if let Some(FieldHwKind::Interrupt(info)) = field.hw_kind.first() {
                            if !info.is_level() {
                                field_entry.push(
                                    SignalInfo::new_with_en_clr(
                                        &format!("{group_name}_d1{reg_idxb}.{field_name}"),
                                        width,
                                        "1'b0",
                                        &format!("{group_name}_l{reg_idxb}.{field_name}"),
                                        &enable, &clear)
                                    );
                            }
                        }
                    }
                    // Create one process for each pair of clock/reset found in the register field
                    for ((clk,rst_name),sig_list) in signals.items() {
                        let mut proc_name = format!("proc_{reg_name}");
                        // Append clk/rst_name to process if different from the register default
                        if clk!=reg_clk && signals.len() > 1 {
                            proc_name.push_str(&format!("_{}",clk));
                        }
                        let mut rst = if clk==&rif.sw_clocking.clk || rif.hw_clocking.is_empty() {&rif.sw_clocking.rst} else {&rif.hw_clocking.first().unwrap().rst};
                        // Find the full reset definition in the sw_clock or hw_clocking
                        if rst_name!=&rst.name {
                            proc_name.push_str(&format!("_{}",rst_name));
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
                        self.gen_process(clk, rst, &proc_name, sig_list);
                    }
                }

                // Create process to generate register pulse access
                if !group_done.contains(&group_name_i) {
                    group_done.insert(group_name_i.clone());

                    let mut signals: Vec<SignalInfo> = Vec::new();
                    let mut reg_clk = "".to_owned();
                    for ctrl in reg_impl.regs_ctrl.iter() {
                        let mut base_name = format!("rif_{group_name_i}.p");
                        let ctrl_name = format!("{}{reg_idx}",ctrl.name.to_casing(Snake));
                        let base_value = format!("{ctrl_name}__decode & if_rif.en");
                        if reg_impl.is_multi_pulse() {
                            base_name.push('_');
                            base_name.push_str(&ctrl_name);
                        };
                        for pulse in ctrl.pulse.iter() {
                            let mut name = base_name.to_owned();
                            let mut value = base_value.to_owned();
                            let p_clk =
                                match pulse {
                                    RegPulseKind::Write(clk)  => {
                                        name.push_str("_write");
                                        value.push_str(" & ~if_rif.rd_wrn");
                                        clk
                                    },
                                    RegPulseKind::Read(clk)   => {
                                        name.push_str("_read");
                                        value.push_str(" & if_rif.rd_wrn");
                                        clk
                                    },
                                    RegPulseKind::Access(clk) => {
                                        name.push_str("_access");
                                        clk
                                    },
                                };
                            // No clock means the pulse is just combinatorial logic
                            if p_clk.is_empty() {
                                self.write(&format!("   assign {name} = {value};\n"));
                            } else {
                                if reg_clk.is_empty() {
                                    reg_clk = p_clk.to_owned();
                                } else if &reg_clk!=p_clk {
                                    return Err(format!("Only one clock should be used for the register {group_name} pulses").into());
                                }
                                signals.push(SignalInfo::new(&name, 1, "1'b0", &value));
                            }
                        }
                    }
                    if !signals.is_empty() {
                        let proc_name = format!("proc_{group_name}{reg_idx}_special");
                        self.gen_process(&reg_clk, &rif.sw_clocking.rst, &proc_name, &signals);
                    }
                }

                // Interrupt registers signals : clock enable and IRQ
                if reg.is_intr() {
                    let intr_info = reg_impl.intr_info(reg)?;
                    // Clock enable : or of all interrupts events (only the base one, not the alternate)
                    if intr_info.name.is_empty() {
                        self.write(&format!("   assign clk_en_intr_{group_name} ="));
                        if let ClkEn::Signal(clk_en) = &reg_impl.clk_en {
                            self.write(&format!(" {clk_en} || "));
                        } else {
                            self.write("\n      ");
                            for field in reg.fields.iter() {
                                if let Some(FieldHwKind::Interrupt(intr_trig)) = field.hw_kind.first() {
                                    let field_name = field.name().to_casing(Snake);
                                    match intr_trig {
                                        // Level Trigger
                                        InterruptTrigger::High => self.write(&format!("{group_name}.{field_name}!=0 ||\n      ")),
                                        InterruptTrigger::Low  => self.write(&format!("~{group_name}.{field_name}!=0 ||\n      ")),
                                        // Edge trigger : enable on change
                                        _ => self.write(&format!("{0}.{1}!={0}_d1.{1} ||\n      ", group_name, field_name)),
                                    }
                                }
                            }
                        }
                        self.write("if_rif.en;\n\n");
                    }
                    // IRQ: or of all interrupts status and-ed with the mask
                    for field in reg.fields.iter() {
                        let field_name = field.name().to_casing(Snake);
                        self.write(&format!("   assign rif_{group_name}_pending.{field_name} = "));
                        if field.is_disabled() {
                            self.write(&Self::field_reset_str(field, false, &rif_pkg_name, &reg.reg_type));
                        } else {
                            self.write(&format!("rif_{group_name_i}.{field_name}"));
                            if intr_info.mask.is_some() {
                                self.write(&format!(" & rif_{group_name}_mask.{field_name}"));
                            }
                        }
                        self.write(";\n");
                    }
                    self.write(&format!("\n   assign rif_{group_name}_irq = \n"));
                    self.write(
                        &reg.fields.iter().filter(|f| !f.is_disabled())
                            .map(|field| format!("      {}rif_{group_name}_pending.{}",
                                if field.width > 1 {"|"} else {""},
                                field.name().to_casing(Snake)))
                            .collect::<Vec<String>>()
                            .join(" ||\n")
                        );
                    self.write(";\n\n");
                }

                // Concatenation for Read data
                self.write(&format!("   assign {reg_name}__read_data = "));
                let nb_fields = reg.fields.iter().rev().filter(|f| !f.sw_kind.is_wo()).count();
                let first_field = reg.fields.iter().find(|f| !f.sw_kind.is_wo());
                let first_width = if let Some(f) = first_field {f.width} else {0};
                let first_is_signed = if let Some(f) = first_field {f.is_signed()} else {false};
                let is_single_wide_field = nb_fields==0 || (nb_fields == 1 && first_width==rif.data_width);
                // Start concatenation of fields if more than one, or cast to unsigned if only one field signed
                if !is_single_wide_field {
                    self.write("{");
                } else if first_is_signed {
                    self.write("$unsigned(");
                }
                let mut prev_lsb = rif.data_width;
                for field in reg.fields.iter().rev().filter(|f| !f.sw_kind.is_wo()) {
                    let field_impl = reg_impl.get_field(&field.name)?;
                    let field_name = field.name().to_casing(Snake);
                    // Fill register spaces with 0s
                    if field.msb() >= prev_lsb {println!("ERROR : Field {} ({:?}) has range [{}:{}] while previous LSB is {prev_lsb}", field.name, field.array, field.msb(), field.lsb);}
                    let spaces = prev_lsb.saturating_sub(field.msb()+1);
                    if spaces != 0 {
                        self.write(&format!("{}'b0,", spaces));
                    }
                    if !reg.is_external() && field_impl.is_local() && field.has_write_mod() {
                        self.write(&format!("{group_name}{intr_suffix}{reg_idx}_{}__reg", field.name_flat().to_casing(Snake)));
                    } else if let FieldSwKind::Password(info) = &field.sw_kind {
                        if info.has_hold() {
                            self.write(&format!("{2}'b0,rif_{0}.{1}_hold,rif_{0}.{1}_locked", group_name_i, field.name, field.width-2));
                        } else {
                            self.write(&format!("{2}'b0,rif_{0}.{1}_locked", group_name_i, field.name, field.width-1));
                        }
                    } else {
                        if !reg.is_external() && (field_impl.is_sw_write() || field.is_hw_write() || field_impl.is_constant()) {
                            self.write("rif_");
                        }
                        self.write(&format!("{group_name}{intr_suffix}{reg_idxb}.{field_name}"));
                    }
                    if let Some(partial_pos) = field.partial.0 {
                        if field.width > 1 {
                            self.write(&format!("[{}:{}]", partial_pos + field.width as u16 - 1, partial_pos));
                        } else {
                            self.write(&format!("[{}]", partial_pos));
                        }
                    }

                    prev_lsb = field.lsb;
                    if prev_lsb!=0 {
                        self.write(",");
                    }
                }
                // Handle case where the first field does not starts at 0
                if prev_lsb!=0 {
                    self.write(&format!("{}'b0", prev_lsb));
                }
                // Close concatenation of fields or unsigned cast
                if !is_single_wide_field {
                    self.write("}");
                }  else if first_is_signed {
                    self.write(")");
                }
                self.write(";\n\n");
            }
        }

        // Handle case of missing fields in a register implementation
        for (group_name, hw_reg) in rif.hw_regs.items() {
            // Skip register if read-only from firmware
            let reg_impl = rif.get_hw_reg(&hw_reg.group);
            if !reg_impl.port.is_out() && reg_impl.interrupt.is_empty() {continue;}
            for (field_name,info) in &hw_reg.missing_fields {
                let rst = Self::value_to_str(info.reset, info.width, info.signed, info.width > 16);
                self.write(&format!("   assign rif_{group_name}.{field_name} = {rst};\n",));
            }
        }

        self.write(&format!("\nendmodule : {rif_name}\n"));

        // Write file
        self.save(&format!("{}.sv", rif.name(false).to_lowercase()))?;
        Ok(())
    }

    fn add_clocking_port(
        &mut self,
        info: &ClockingInfo,
        list: &mut HashSet<String>,
        is_hw: bool,
    ) {
        let kind = if is_hw { "Hardware" } else { "Software" };
        // Clock
        if !list.contains(&info.clk) {
            self.write(&format!("   input var logic {}, // {kind} Clock \n", info.clk));
            list.insert(info.clk.to_owned());
        }
        // Reset
        if !list.contains(&info.rst.name) {
            self.write(&format!(
                "   input var logic {}, // {kind} {}\n",
                info.rst.name, info.rst.desc()
            ));
            list.insert(info.rst.name.to_owned());
        }
        // Clear
        if !info.clear.is_empty() && !list.contains(&info.clear) {
            self.write(&format!("   input var logic {}, // {kind} Clear\n", info.clear));
            list.insert(info.clear.to_owned());
        }
    }

    fn add_intf(&mut self, intf: &Interface, addr_w: u8, data_w: u8) {
        let ports = RifIntfPorts::new(intf);
        let mut ports_iter = ports.iter().peekable();
        while let Some(port) = ports_iter.next()  {
            self.write_port(port, None, addr_w, data_w, false, ports_iter.peek().is_none());
        }
    }

    /// Write a port declaration
    fn write_port(&mut self, port: &PortInfo, prefix: Option<&String>, addr_w: u8, data_w: u8, is_ctrl: bool, is_last: bool) {
        let dir = match &port.dir {
            PortDir::In  => if is_ctrl {"output"} else {"input"},
            PortDir::Out => if is_ctrl {"input"} else {"output"},
            PortDir::Modport((rif,ctrl)) => if is_ctrl {ctrl} else {rif},
        };
        self.write("   "); // Indentation
        match &port.width {
            PortWidth::Custom(type_name) => {
                if port.is_intf() {
                    self.write(&format!("{type_name}.{dir} "));
                } else {
                    self.write(&format!("{dir} var {type_name} "));
                }
            }
            _ => {
                self.write(&format!("{dir} var logic "));
                let w = port.width(addr_w, data_w);
                if w > 1 {
                    self.write(&format!("[{}:0] ", w-1));
                }
            }
        }
        if let Some(prefix) = prefix {
            self.write("_");
            self.write(prefix);
        }
        self.write(&port.name.to_casing(Snake));
        // Write separator
        self.write(if is_last {" "} else {","});
        // Write comment if any
        if !port.desc.is_empty() {
            self.write(" // ");
            self.write(&port.desc);
        }
        self.write("\n");

    }

    fn add_intf_bridge(&mut self, intf: &Interface, addr_w: u8, data_w: u8, sw_clk: &str, sw_rst: &str) {
        if intf.is_default() {
            return;
        }
        self.write("\n/*------------------------------------------------------------------------------\n");
        self.write("--  Bridge to the internal register interface\n");
        self.write("------------------------------------------------------------------------------*/\n");
        self.write(&format!("   rif_if#({addr_w}, {data_w}) if_rif({sw_clk}, {sw_rst});\n"));
        self.write("\n");
        let name = intf.name();
        self.write(&format!("   bridge_{name}_rif#({addr_w}, {data_w}) i_bridge(.*);\n"));
    }


    pub fn get_signal_name(val: &Option<String>, ext: &str, group_type: &str, group_name: &str, reg_idx: &str, field_name: &str, field_idx: &str) -> String {
        let empty_str = "".to_owned();
        let name = val.as_ref().unwrap_or(&empty_str);
        if name.starts_with('(') {
            return name.to_owned();
        }
        let mut parts = name.split('.');
        match (parts.next(),parts.next()) {
            // No register name, or the register name match the type: local field
            (Some(f),None) if !f.is_empty() => format!("{group_name}.{f}"),
            (Some(r),Some(f)) if r == group_type || r == "this" || r == "self" => format!("{group_name}.{f}"),
            // Format ".name" : input port
            (Some(r),Some(n)) if r.is_empty() =>n.to_owned(),
            // Esternal field
            (Some(r),Some(f)) => format!("{r}.{f}"),
            // No name provided: use default naming
            _ => format!("{group_name}{reg_idx}.{field_name}{ext}{field_idx}"),
        }
    }

    pub fn add_cast(val: &str, field: &RifFieldInst, pkg_name: &str, reg_type: &str) -> String {
        if let EnumKind::Type(enum_type) = &field.enum_kind {
            let etn =
                match enum_type {
                    _ if enum_type == "type" => format!("{pkg_name}_pkg::e_{reg_type}_{}", field.name),
                    t if !enum_type.contains("::") => format!("{pkg_name}_pkg::{t}"),
                    t => t.to_owned()
                };
            format!("{etn}'({val})")
        } else if field.is_signed() {
            format!("$signed({val})")
        } else {
            val.to_owned()
        }
    }

    pub fn value_to_str<T>(val: T, width: u16, is_signed: bool, is_hexa: bool) -> String
    where T: std::fmt::Display + std::fmt::LowerHex + From<u128> + std::ops::Shl<u16, Output = T> + std::ops::Sub<Output = T> + PartialOrd {
        match (is_hexa, is_signed) {
            (true, true)   => format!("{width}'sh{val:x}"),
            (true, false)  => format!("{width}'h{val:x}"),
            (false, true)  => {
                let max : T = (1 << (width-1)).into();
                if val >= max {
                    format!("-{width}'sd{}", (max << 1) - val)
                } else {
                    format!("{width}'sd{val}")
                }
            },
            (false, false) => format!("{width}'d{val}"),
        }
    }

    pub fn field_reset_str(field: &RifFieldInst, is_hexa: bool, pkg_name: &str, reg_type: &str) -> String {
        let val = field.reset.to_u128(field.width);
        if let EnumKind::Type(enum_type) = &field.enum_kind {
            let etn =
                match enum_type {
                    _ if enum_type == "type" => format!("{pkg_name}_pkg::e_{reg_type}_{}", field.name),
                    t if !enum_type.contains("::") => format!("{pkg_name}_pkg::{t}"),
                    t => t.to_owned()
                };
            format!("{etn}'({val})")
        } else {
            Self::value_to_str(val, field.width.into(), field.is_signed(), is_hexa || field.width > 16)
        }
    }

    /// Generate synchronous process
    fn gen_process(&mut self, clk: &str, rst: &ResetDef, name: &str, signals: &[SignalInfo]) {
        // Check all signals clear/enable o see if all signals share a condition or not
        let mut signals_iter = signals.iter();
        let (clk_en, clr): (Option<&String>, Option<&String>) =
            if let Some(signal) = signals_iter.next() {
                (signal.enable.as_ref(), signal.clear.as_ref())
            } else {
                (None, None)
            };
        let mut clk_en_global = true;
        let mut clr_global = true;
        for signal in signals_iter {
            if clk_en_global {
                if let Some(n) = &signal.enable {
                    if clk_en.is_none() || n != clk_en.unwrap() {
                        clk_en_global = false;
                    }
                } else if clk_en.is_some() {
                    clk_en_global = false;
                }
            }
            if clr_global {
                if let Some(n) = &signal.clear {
                    if clr.is_none() || n != clr.unwrap() {
                        clr_global = false;
                    }
                } else if clr.is_some() {
                    clr_global = false;
                }
            }
        }
        // Declaration
        self.write(&format!("\n   always_ff @(posedge {clk}"));
        if !rst.sync {
            let pol = if rst.active_high { "pos" } else { "neg" };
            self.write(&format!(" or {pol}edge {}", rst.name));
        }
        // Reset
        self.write(&format!(") begin : {name}\n      if("));
        if !rst.active_high {
            self.write("!");
        }
        self.write(&format!("{}) begin\n", rst.name));
        for signal in signals.iter() {
            self.write(&format!("         {} <= {};\n", signal.name, signal.reset));
        }
        self.write("      end else ");
        // Optional Global Enable
        // Should the clear be included in the enable condition ? controllable ?
        if clk_en_global && clk_en.is_some() {
            self.write(&format!("if({}) ", clk_en.unwrap()));
        }
        self.write("begin\n");
        // Optional Global clear
        if clr_global && clr.is_some() {
            self.write(&format!("      if({}) begin\n", clr.unwrap()));
            for signal in signals.iter() {
                self.write(&format!("            {} <= {};\n", signal.name, signal.reset));
            }
            self.write("      end else begin\n");
        }
        // Set value
        for signal in signals.iter() {
            self.write("         ");
            if !clr_global && signal.clear.is_some() {
                self.write(&format!(
                    "if({})\n            {} <= {};\n            else",
                    signal.clear.as_ref().unwrap(),
                    signal.name,
                    signal.reset
                ));
            }
            if !clk_en_global && signal.enable.is_some() {
                self.write(&format!(
                    "if({})\n            ",
                    signal.enable.as_ref().unwrap()
                ));
            }
            self.write(&format!("{} <= {};\n", signal.name, signal.value));
        }
        //
        if clr_global && clr.is_some() {
            self.write("      end\n");
        }
        self.write("      end\n   end\n\n");
    }


    //-----------------------------------------------------------------------------
    // RIF Mux implementation: Address decoding, registers , ...
    //-----------------------------------------------------------------------------

    fn gen_rifmux(&mut self, rifmux: &RifmuxInst) -> Result<(), Box<dyn std::error::Error>> {
        let msb = rifmux.addr_width - 1;
        let name_len = rifmux.components.iter().map(|c| c.get_name().len()).max().unwrap_or(0);

        // Header (TODO: support external template)
        self.write("// File generated automatically: DO NOT EDIT.\n\n");
        // TODO: handle suffix/Prefix
        let rifmux_name = &rifmux.inst_name;
        self.write(&format!("module {rifmux_name}"));

        // Port declaration
        self.write(" (\n");
        if !rifmux.interface.is_default() {
            self.write(&format!("   input var logic {}, // Bridge clock \n", rifmux.sw_clocking.clk));
            self.write(&format!("   input var logic {}, // Bridge reset: {}\n",
                rifmux.sw_clocking.rst.name, rifmux.sw_clocking.rst.desc()
            ));
        }
        for comp in rifmux.components.iter() {
            self.write(&format!("   rif_if.ctrl if_{0:<1$}, // {2}\n", comp.get_name(), name_len, comp.get_desc_short()));
        }
        self.add_intf(&rifmux.interface, rifmux.addr_width, rifmux.data_width);
        self.write(");\n\n");

        self.write("   logic addr_invalid; // High when address is not in the range of any of the connected RIF\n");
        self.write("   logic addr_invalid_next; // Combinatorial version of addr_invalid\n");

        // Add interface bridge when not default
        self.add_intf_bridge(&rifmux.interface, rifmux.addr_width, rifmux.data_width, &rifmux.sw_clocking.clk, &rifmux.sw_clocking.rst.name);

        // Address demultiplexing
        self.write("\n/*------------------------------------------------------------------------------\n");
        self.write("--  Demux access\n");
        self.write("------------------------------------------------------------------------------*/\n\n");

        let mut en_names = Vec::new();
        for comp in rifmux.components.iter() {
            let name = comp.get_name();
            let lsb = comp.get_addr_width();
            let addr_map = comp.addr >> lsb;

            self.write(&format!("   // {}\n", name.to_casing(Title)));
            // Enable : high when main enable is high and address match
            let en = format!("if_{name}.en");
            self.write(&format!("   assign {en:<0$} = if_rif.en && if_rif.addr[{msb}:{lsb}]=={addr_map};\n",name_len+11));
            en_names.push(en);
            // Address : Forced to 0 when address is not matching
            let addr = format!("if_{name}.addr");
            self.write(&format!("   assign {addr:<0$} = if_rif.addr[{msb}:{lsb}]=={addr_map} ? if_rif.addr[{1}:0] : {lsb}'b0;\n",name_len+11, lsb-1));
            // Write data : just copy the main interface
            let data = format!("if_{name}.wr_data");
            self.write(&format!("   assign {data:<0$} = if_rif.wr_data;\n",name_len+11));
            // Read/Write control : just copy the main interface
            let rd_wrn = format!("if_{name}.rd_wrn");
            self.write(&format!("   assign {rd_wrn:<0$} = if_rif.rd_wrn;\n\n",name_len+11));
        }

        // Address demultiplexing
        self.write("/*------------------------------------------------------------------------------\n");
        self.write("--  Mux feedback\n");
        self.write("------------------------------------------------------------------------------*/\n\n");

        self.write("   assign addr_invalid_next = if_rif.en & ~(");
        self.write(&en_names.join(" | "));
        self.write(");\n");

        // TODO : Use argument to insert pipe
        // let sig_add_invalid = vec![SignalInfo::new("addr_invalid",1,ResetVal::Unsigned(0),"addr_invalid_next")];
        // gen_process(&mut txt, &def.sw_clocking.clk, &def.sw_clocking.rst, "proc_addr_invalid",&sig_add_invalid);
        self.write("   assign addr_invalid = addr_invalid_next;\n\n");


        self.write("   assign if_rif.done = addr_invalid |\n      ");
        self.write(&rifmux.components.iter()
            .map(|c| format!("if_{}.done{:<2$}", c.inst.get_name(), "", name_len-c.get_name().len()))
            .collect::<Vec<String>>()
            .join(" |\n      "));
        self.write(" ;\n");

        self.write("   assign if_rif.done_next = addr_invalid_next |\n      ");
        self.write(&rifmux.components.iter()
            .map(|c| format!("if_{}.done_next{:<2$}", c.inst.get_name(), "", name_len-c.get_name().len()))
            .collect::<Vec<String>>()
            .join(" |\n      "));
        self.write(" ;\n\n");

        self.add_mux_if(&rifmux.components, "rd_data", name_len, "0");
        self.add_mux_if(&rifmux.components, "err_addr", name_len, "1'b1");
        self.add_mux_if(&rifmux.components, "err_access", name_len, "1'b0");
        self.add_mux_if(&rifmux.components, "err_addr_next", name_len, "1'b1");
        self.add_mux_if(&rifmux.components, "err_access_next", name_len, "1'b0");

        self.write(&format!("endmodule : {rifmux_name}\n"));

        // Write file
        self.save(&format!("{}.sv", rifmux.type_name))
    }

    pub fn add_mux_if(&mut self, comps: &Vec<CompInst>, name: &str, len: usize, err_val: &str) {
        let suffix = if name.ends_with("_next") {"_next"} else {""};
        self.write(&format!("   assign if_rif.{name} = addr_invalid{suffix} ? {err_val} :\n"));
        let pad = "";
        for (i,comp) in comps.iter().enumerate() {
            let top = comp.get_name();
            let nb = len - top.len();
            if i != comps.len() - 1 {
                self.write(&format!("      if_{top}.done{suffix}{pad:<nb$} ? if_{top}.{name}{pad:<nb$} :\n"));
            } else {
                self.write(&format!("      {pad:<0$}   if_{top}.{name}{pad:<nb$} ;\n\n", len+8+suffix.len()));
            }
        }
    }

    fn gen_rifmux_pkg(&mut self, rifmux: &RifmuxInst) -> Result<(), Box<dyn std::error::Error>> {
        let name_len = rifmux.components.iter().map(|c| c.get_name().len()).max().unwrap_or(0);
        // Header (TODO: support external template)
        self.write("// File generated automatically: DO NOT EDIT.\n\n");
        self.write(&format!("package {}_pkg;\n\n", rifmux.type_name));
        for comp in rifmux.components.iter() {
            let w = ((rifmux.addr_width+3)>>2) as usize;
            let pad = name_len - comp.get_name().len();
            self.write(&format!("   localparam logic [{}:0] {}_BASE_ADDR{:<pad$} = {}'h{:0w$x};\n",
                rifmux.addr_width-1,
                comp.get_name().to_uppercase(),
                "",
                rifmux.addr_width,
                comp.addr
            ));
                // .format(rifmux['addrWidth']-1,k.upper(),rifmux['addrWidth'],v['addr'],int(rifmux['addrWidth']/4))
        }
        self.write(&format!("\nendpackage : {}_pkg\n", rifmux.type_name));

        // Write file
        self.save(&format!("{}_pkg.sv", rifmux.type_name))

    }


    //-----------------------------------------------------------------------------
    // RIF Top implementation: instantiate rifmux + rif
    //-----------------------------------------------------------------------------
    fn gen_riftop(&mut self, rifmux: &RifmuxInst) -> Result<(), Box<dyn std::error::Error>> {
        let Some(riftop) = &rifmux.top else { return Ok(())};
        let riftop_name = riftop.name.to_casing(Snake);

        let sw_clk = &rifmux.sw_clocking.clk;
        let sw_rst = &rifmux.sw_clocking.rst.name;
        let intf_ports = RifIntfPorts::new(&rifmux.interface);

        // Header (TODO: support external template)
        self.write("// File generated automatically: DO NOT EDIT.\n\n");

        // Module declaration
        self.write(&format!("module {riftop_name} (\n"));
        // Clocks and reset
        self.write("   // RTL clock/Reset\n");
        self.write(&format!("   input var logic {sw_clk}, // Software clock \n"));
        self.write(&format!("   input var logic {sw_rst}, // Software reset: {}\n", rifmux.sw_clocking.rst.desc()));
        self.names.clear();
        self.names.push(sw_clk.to_owned());
        self.names.push(sw_rst.to_owned());
        let mut nb_ctrl = 0;
        for rif in rifmux.components.iter().filter_map(|c| c.get_rif()) {
            nb_ctrl += rif.ports.clk_ens.len() + rif.ports.ctrls.len();
            for clk in rif.ports.clocks.iter().skip(1) {
                if !self.names.contains(&clk.name) {
                    self.names.push(clk.name.to_owned());
                    self.write_port(clk, None, 0, 0, false, false);
                }
            }
            for rst in rif.ports.resets.iter().skip(1) {
                if !self.names.contains(&rst.name) {
                    self.names.push(rst.name.to_owned());
                    self.write_port(rst, None, 0, 0, false, false);
                }
            }
        }
        // Controls: clock enables, clear, lock, ...
        if nb_ctrl > 0 {
            self.write("   // Controls\n");
            for rif in rifmux.components.iter().filter_map(|c| c.get_rif()) {
                for port in rif.ports.clk_ens.iter() {
                    if !self.names.contains(&port.name) {
                        self.write_port(port, None, rif.addr_width, rif.data_width, false, false);
                    }
                }
                for port in rif.ports.ctrls.iter() {
                    if !self.names.contains(&port.name) {
                        self.write_port(port, None, rif.addr_width, rif.data_width, false, false);
                    }
                }
            }
        }
        // Register of each instances
        for rif in rifmux.components.iter().filter_map(|c| c.get_rif()) {
            let prefix = riftop.prefixes.get(&rif.inst_name);
            self.write(&format!("   // {} registers\n", rif.name(false).to_casing(Title)));
            for port in rif.ports.regs.iter().filter(|p| p.dir.is_in()) {
                self.write_port(port, prefix, rif.addr_width, rif.data_width, false, false);
            }
            for port in rif.ports.regs.iter().filter(|p| p.dir.is_out()) {
                self.write_port(port, prefix, rif.addr_width, rif.data_width, false, false);
            }
            for port in rif.ports.irqs.iter() {
                self.write_port(port, prefix, rif.addr_width, rif.data_width, false, false);
            }
        }
        // Control interface
        self.write("   // Control Interface\n");
        self.add_intf(&rifmux.interface, rifmux.addr_width, rifmux.data_width);
        self.write(");\n");

        // Interface declaration
        self.write("\n/*------------------------------------------------------------------------------\n");
        self.write("--  Interfaces to sub-RIF\n");
        self.write("------------------------------------------------------------------------------*/\n");

        let data_w = rifmux.data_width;
        for comp in rifmux.components.iter() {
            let name = comp.get_name().to_casing(Snake);
            let addr_w = comp.get_addr_width();
            self.write(&format!("   rif_if#({addr_w}, {data_w}) if_{name}({sw_clk}, {sw_rst});\n"));
        }

        // Instances RIF MUX and all RIFs
        self.write("\n/*------------------------------------------------------------------------------\n");
        self.write("--  Instances\n");
        self.write("------------------------------------------------------------------------------*/\n");

        // RifMux
        self.write(&format!("   {} i_rifmux (\n", rifmux.type_name.to_casing(Snake)));
        // SW interface
        self.write(&format!("      .{sw_clk}({sw_clk}),\n"));
        self.write(&format!("      .{sw_rst}({sw_rst}),\n"));
        for p in intf_ports.iter() {
            self.write(&format!("      .{0}({0}),\n", p.name));
        }
        // RIFs interface
        let mut comp_iter = rifmux.components.iter().peekable();
        while let Some(comp) = comp_iter.next()  {
            let name = comp.get_name().to_casing(Snake);
            self.write(&format!("      .if_{name}(if_{name})"));
            if comp_iter.peek().is_some() {
                self.write(",");
            }
            self.write("\n");
        }
        self.write("   );\n\n");

        // RIFs
        for comp in rifmux.components.iter().filter(|c| !c.is_external()) {
            let inst_name = comp.get_name().to_casing(Snake);
            let type_name = comp.get_type().to_casing(Snake);
            let prefix = riftop.prefixes.get(comp.get_name())
                .map(|p| format!("{p}_"))
                .unwrap_or("".to_owned());
            self.write(&format!("   {type_name} i_{inst_name} (\n"));
            match &comp.inst {
                Comp::Rif(rif) => {
                    if let Some(clk) = rif.ports.clocks.first() {
                        self.write(&format!("      .{}({sw_clk}),\n", clk.name));
                    }
                    if let Some(rst) = rif.ports.resets.first() {
                        self.write(&format!("      .{}({sw_rst}),\n", rst.name));
                    }
                    for p in rif.ports.clocks.iter().skip(1) {
                        self.write(&format!("      .{0}({0}),\n", p.name));
                    }
                    for p in rif.ports.resets.iter().skip(1) {
                        self.write(&format!("      .{0}({0}),\n", p.name));
                    }
                    for p in rif.ports.clk_ens.iter() {
                        self.write(&format!("      .{0}({0}),\n", p.name));
                    }
                    for p in rif.ports.ctrls.iter() {
                        self.write(&format!("      .{0}({0}),\n", p.name));
                    }
                    for p in rif.ports.regs.iter().filter(|p| p.dir.is_in()) {
                        self.write(&format!("      .{0}({prefix}{0}),\n", p.name));
                    }
                    for p in rif.ports.regs.iter().filter(|p| p.dir.is_out()) {
                        // Output port are prefixed by rif_ : remove it to insert the configured prefixed
                        let name_base = p.name.strip_prefix("rif_").unwrap_or(&p.name);
                        self.write(&format!("      .{0}(rif_{prefix}{name_base}),\n", p.name));
                    }
                    for p in rif.ports.irqs.iter() {
                        self.write(&format!("      .{0}({0}),\n", p.name));
                    }
                }
                Comp::Rifmux(_) => return Err("Rifmux inside RIF top not supported yet".into()),
                _ => unreachable!()
            }
            //
            self.write(&format!("      .if_rif(if_{inst_name})\n"));
            self.write("   );\n\n");
        }


        self.write(&format!("endmodule : {riftop_name}"));

        // Write file
        self.save(&format!("{}.sv", riftop_name))
    }

}