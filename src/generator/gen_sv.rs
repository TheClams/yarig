use crate::{
    cfg::{CfgRtl, RtlLimit, RtlLimitCfg}, comp::{comp_inst::RifInst, hw_info::{PortDir, PortInfo, SignalDecl, SignalDef, SignalInfo, SignalKind}}, rifgen::{CastInfo, EnumEntry, ExprId, GenericRange, LogicExpr, ResetDef, SignalRange}
};

use super::{
    gen_common::{CompInfo, GeneratorBase, GeneratorBaseSetting, GeneratorCore},
    trait_hw::GeneratorHw
};

use yarig_macro::add_gen_core;
#[add_gen_core("sv")]
#[allow(dead_code)]
pub struct GeneratorSv {
    /// Number of pipe level for register access (default 1 on the read value)
    nb_pipe: u8,
    /// Generate constant for register address in the package
    const_reg: bool,
    /// Generate constant for field reset/position/width
    const_field: bool,
    /// True when current module instance is a bridge
    is_bridge : bool,
    /// Controls how field limits are used
    limit: RtlLimitCfg,
    /// add a pipe level to the generation of invalid address flag when no RIF is selected
    rifmux_pipe_invalid: bool,
    /// Software clock for curren RIF
    sw_clk: String,
    /// Flag when UVM library should be imported for current RIF
    import_uvm: bool,
}

impl GeneratorSv {

    pub fn new(setting: GeneratorBaseSetting, extra: CfgRtl) -> Self {
        let mut core = GeneratorCore::new(0,setting);
        // Override gen_inc if defined in the extra settings
        if let Some(gen_inc) = extra.gen_inc {
            core.setting.gen_inc = gen_inc;
        }
        let limit = RtlLimitCfg::new(extra.limit, extra.force_limit);
        GeneratorSv {
            core,
            nb_pipe: extra.nb_pipe.unwrap_or(1),
            const_reg: extra.const_reg.unwrap_or(false),
            const_field: extra.const_field.unwrap_or(false),
            rifmux_pipe_invalid: extra.rifmux_pipe_invalid.unwrap_or(false),
            is_bridge: false,
            limit,
            sw_clk: "clk".to_owned(),
            import_uvm: false
        }
    }

    fn add_signal_def(&mut self, def: &SignalDef, prefix: Option<&String> ) {
        match &def.kind {
            SignalKind::Unsigned(w) => {
                self.write("logic ");
                if *w > 1 {
                    self.write(&format!("[{}:0] ", w-1));
                }
            },
            SignalKind::Signed(w)   => self.write(&format!("logic signed [{}:0] ", w-1)),
            SignalKind::Custom((Some(pkg),n)) => self.write(&format!("{pkg}::{n} ")),
            SignalKind::Custom((None,n))      => self.write(&format!("{n} ")),
            SignalKind::Integer     => self.write("int "),
            SignalKind::Address     => self.write(&format!("logic [{}:0] ", self.addr_width()-1)),
            SignalKind::Data        => self.write(&format!("logic [{}:0] ", self.data_width()-1)),
        }
        if let Some(prefix) = prefix {
            if let Some(base_name) = def.name.strip_prefix("rif_") {
                self.write(&format!("rif_{prefix}_{}", base_name.trim()));
            } else {
                self.write(&format!("{prefix}_{}",def.name.trim()));
            }
        } else {
            self.write(def.name.trim());
        }
        if def.is_array() {
            self.write(&format!("[{}]", def.dim));
        }
    }

    fn write_signal_seq(&mut self, id: &ExprId, val: &LogicExpr, lvl: usize) {
        self.write(&" ".repeat(3*lvl));
        self.add_expr_id(id);
        self.write(" <= ");
        self.add_logic_expr(val, 0, false);
        self.write(";\n");
    }

    fn add_logic_expr(&mut self, expr: &LogicExpr, lvl: usize, is_part: bool) {
        self.core.write(&" ".repeat(3*lvl));
        match expr {
            LogicExpr::Id(expr_id) => self.add_expr_id(expr_id),
            LogicExpr::Cast(cast_info, e) => {
                let has_par = !cast_info.is_none();
                match cast_info {
                    CastInfo::None => {}
                    CastInfo::Unsigned => self.core.write("$unsigned("),
                    CastInfo::Signed => self.core.write("$signed("),
                    CastInfo::Custom(pkg, name) => {
                        if !pkg.is_empty() {
                            self.core.write(pkg);
                            self.core.write("::");
                        }
                        self.core.write(name);
                        self.core.write("'(");
                    }
                }
                self.add_logic_expr(e, 0, false);
                if has_par {
                    self.core.write(")");
                }
            }
            LogicExpr::CastFrom(_,w, e) => {
                self.core.write(&format!("{w}'("));
                self.add_logic_expr(e, 0, is_part);
                self.core.write(")");
            }
            LogicExpr::ValueU(v, w) => {
                let n = (w+3)>>2;
                match w {
                    0 => self.core.write(&format!("'{v}")),
                    1 => self.core.write(&format!("1'b{v}")),
                    2..=12 => self.core.write(&format!("{w}'d{v}")),
                    _ => self.core.write(&format!("{w}'h{v:0n$x}")),
                }
            }
            LogicExpr::ValueI(v, w) => {
                let n = (w+3)>>2;
                match w {
                    0 => self.core.write(&format!("{v}")),
                    1..=12 => {
                        if *v < 0 {
                            self.core.write(&format!("-{w}'sd{}", v.abs()))
                        } else {
                            self.core.write(&format!("{w}'sd{v}"))
                        }
                    }
                    _ => self.core.write(&format!("{w}'sh{v:0n$x}")),
                }
            }
            LogicExpr::Not(e) => {
                self.core.write("!");
                self.add_logic_expr(e, 0, true);
            }
            LogicExpr::NotB(e) => {
                self.core.write("~");
                self.add_logic_expr(e, 0, true);
            }
            LogicExpr::Concat(exprs) => {
                self.core.write("{");
                let mut expr_iter = exprs.iter().peekable();
                while let Some(e) = expr_iter.next() {
                    self.add_logic_expr(e, 0, false);
                    if expr_iter.peek().is_some() {
                        self.core.write(",");
                    }
                }
                self.core.write("}");
            }
            LogicExpr::Add(lhs, rhs)  => self.add_two_expr(lhs, rhs, " + " , false),
            LogicExpr::Sub(lhs, rhs)  => self.add_two_expr(lhs, rhs, " - " , false),
            LogicExpr::Eq(lhs, rhs)   => self.add_two_expr(lhs, rhs, " == ", false),
            LogicExpr::Neq(lhs, rhs)  => self.add_two_expr(lhs, rhs, " != ", false),
            LogicExpr::Gte(lhs, rhs)  => self.add_two_expr(lhs, rhs, " >= ", false),
            LogicExpr::Gt (lhs, rhs)  => self.add_two_expr(lhs, rhs, " > " , false),
            LogicExpr::Lte(lhs, rhs)  => self.add_two_expr(lhs, rhs, " <= ", false),
            LogicExpr::Lt(lhs, rhs)   => self.add_two_expr(lhs, rhs, " < " , false),
            LogicExpr::Xor(lhs, rhs)  => self.add_two_expr(lhs, rhs, " ^ ", is_part),
            LogicExpr::OrB(lhs, rhs)  => self.add_two_expr(lhs, rhs, " | ", is_part),
            LogicExpr::AndB(lhs, rhs) => self.add_two_expr(lhs, rhs, " & ", is_part),
            LogicExpr::Or(vec)  => self.add_n_expr(vec, " || ", is_part, lvl),
            LogicExpr::And(vec) => self.add_n_expr(vec, " && ", is_part, lvl),
            LogicExpr::Ite(it_exprs, else_expr) => {
                let tab = if lvl>0 {format!("\n{}", " ".repeat(3*lvl))} else {" ".to_owned()};
                if is_part {self.core.write("(");}
                for it_expr in it_exprs {
                    self.add_logic_expr(&it_expr.0, 0, false /*it_expr.0.nb() > 1*/);
                    self.core.write(" ? ");
                    self.add_logic_expr(&it_expr.1, 0, it_expr.1.nb() > 1 || it_expr.1.is_ite() );
                    self.core.write(" :");
                    self.core.write(&tab);
                }
                self.add_logic_expr(else_expr, 0, else_expr.nb() > 1);
                if is_part {self.core.write(")");}
            }
        }
    }

    fn add_two_expr(&mut self, lhs: &LogicExpr, rhs: &LogicExpr, op: &str, is_part: bool) {
        if is_part {self.write("(");}
        self.add_logic_expr(lhs, 0, true);
        self.write(op);
        if rhs.is_ite() {
            self.write("\n      ");
        }
        self.add_logic_expr(rhs, 0, true);
        if is_part {self.write(")");}
    }

    fn add_n_expr(&mut self, exprs: &[LogicExpr], op: &str, is_part: bool, lvl: usize) {
        let is_part = is_part && exprs.len() > 1;
        let eol = if !is_part && lvl > 0 && exprs.len() > 2 {
            format!("\n{}", " ".repeat(3*lvl))
        } else {
            "".to_owned()
        };
        if is_part {self.core.write("(");}
        let mut expr_iter = exprs.iter().peekable();
        while let Some(expr) = expr_iter.next() {
            self.add_logic_expr(expr, 0, true);
            if expr_iter.peek().is_some() {
                self.core.write(op);
                self.core.write(&eol);
            }
        }
        if is_part {self.core.write(")");}
    }

    fn add_expr_id(&mut self, expr: &ExprId) {
        self.core.write(&expr.name);
        if let Some(idx) = expr.idx {
            self.core.write(&format!("[{idx}]"));
        }
        if let Some(field) = &expr.field {
            self.core.write(".");
            self.core.write(field);
        }
        if let Some(r) = &expr.range {
            match r {
                SignalRange::Fixed((msb,lsb)) => {
                    if msb==lsb {
                        self.core.write(&format!("[{lsb}]"));
                    } else {
                        self.core.write(&format!("[{msb}:{lsb}]"));
                    }
                }
                SignalRange::GenericWidth((width,lsb)) => self.core.write(&format!("[{lsb}+:{width}]")),
                SignalRange::GenericLsb((msb,lsb)) => self.core.write(&format!("[{msb}:{lsb}]")),
            }
        }
    }

}

impl GeneratorHw for GeneratorSv {

    /// Flag when register constants (address/reset) should be generated
    fn has_const_reg(&self) -> bool {self.const_reg}

    /// Flag when field constants (mask, lsb, msb, reset) should be generated
    fn has_const_field(&self) -> bool {self.const_field}

    /// Flag when field constants (mask, lsb, msb, reset) should be generated
    fn limit_cfg(&self) -> RtlLimitCfg {
        self.limit
    }

    /// Return the RifMux address invalid pipe configuration
    fn rifmux_pipe_invalid(&self) -> bool {
        self.rifmux_pipe_invalid
    }

    /// Write generic header for a file
    fn write_file_header(&mut self) {
        // Add header : TODO: configurable header
        self.write_comment(0, "File generated automatically: DO NOT EDIT.");
        self.write("\n");
    }

    fn set_rif_info(&mut self, rif: &RifInst) {
        self.sw_clk = rif.sw_clocking.clk.clone();
        self.import_uvm =
            (self.limit.1==RtlLimit::UvmError && !rif.enum_defs.is_empty()) ||
            (self.limit.0==RtlLimit::UvmError && rif.iter_reg().flat_map(|r| r.fields.iter()).any(|f| f.has_limit()));

    }

    fn write_pkg_header(&mut self, name: &str) {
        self.write(&format!("package {name}_pkg;\n\n"));
    }
    fn write_pkg_footer(&mut self, name: &str) {
        self.write(&format!("endpackage : {name}_pkg\n"));
    }

    fn write_const(&mut self, signal: &SignalDecl, value: LogicExpr) {
        self.write("   localparam ");
        match &signal.def.kind {
            SignalKind::Unsigned(1) => self.write("bit "),
            SignalKind::Unsigned(w) => self.write(&format!("logic [{}:0] ", w-1)),
            SignalKind::Integer     => self.write("int "),
            _ => unreachable!("Constant were supposed to be only unsigned or int got {:?}", signal.def.kind)
        }
        self.write(&signal.def.name);
        self.write(" = ");
        match &value {
            LogicExpr::ValueU(v,w) => {
                if signal.name().ends_with("_W") {
                    self.write(&format!("{v:2}"))
                } else if signal.def.kind==SignalKind::Unsigned(1) {
                    self.write(&format!("{v}"))
                } else {
                    let n = (w+3)>>2;
                    self.write(&format!("{w}'h{v:0n$x}"))
                }
            },
            LogicExpr::ValueI(v,w) => {
                if matches!(signal.def.kind,SignalKind::Integer | SignalKind::Unsigned(1)) {
                    self.write(&format!("{v}"))
                } else {
                    let n = (w+3)>>2;
                    self.write(&format!("{w}'sh{v:0n$x}"))
                }
            }
            _ => self.add_logic_expr(&value, 0, false),
        }
        self.write(";\n");
    }

    // Enums declaration
    fn write_enum_header(&mut self, _name: &str, width: u8) {
        self.write("   typedef enum logic ");
        if width > 1 {
            self.write(&format!("[{}:0] ", width - 1));
        }
        self.write("{\n");
    }

    fn write_enum_entry(&mut self, entry: &EnumEntry, is_last: bool) {
        self.write(&format!("      {} = {}", entry.name, entry.value));
        self.write(if is_last {" "} else {","});
        self.write(" // ");
        self.write(&entry.description.get_short(false));
        self.write("\n");
    }

    fn write_enum_footer(&mut self, name: &str, _width: u8) {
        self.write("   } ");
        self.write(name);
        self.write(";\n\n");
    }

    // Structure declaration
    fn write_struct_header(&mut self, _name: &str, fields: &[SignalDecl]) {
        let has_array = fields.iter().any(|f| f.def.is_array());
        let packed = if has_array { "" } else { "packed " };
        self.write(&format!("   typedef struct {packed}{{\n"));
    }

    fn write_struct_field(&mut self, field: &SignalDecl, _is_last: bool) {
        self.write("      ");
        self.add_signal_def(&field.def, None);
        self.write(";");
        if !field.desc.is_empty() {
            self.write(" // ");
            self.write(&field.desc);
        }
        self.write("\n");
    }

    fn write_struct_footer(&mut self, name: &str) {
        self.write(&format!("   }} {name};\n\n"));
    }

    // Module declaration
    fn write_module_decl_header(&mut self, name: &str) {
        self.write(&format!("module {name}"));
    }

    fn write_module_generic_header(&mut self) {
        self.write(" #(\n");
    }

    fn write_module_generic_decl(&mut self, name: &str, range: &GenericRange, is_last: bool) {
        self.write("   parameter bit ");
        if range.max > 1 {
            let msb = (u16::BITS - range.max.leading_zeros()) - 1;
            self.write(&format!("[{msb}:0] "));
        }
        self.write(&format!("{name} = {}", range.default));
        let sep = if is_last {" "} else {","};
        self.write(sep);
        if let Some(desc) = &range.desc {
            self.write(&format!(" // {desc} [{}:{}]", range.min, range.max));
        }
        self.write("\n");
        if is_last {
            self.write(")")
        }
    }

    fn write_module_port_header(&mut self) {
        self.write(" (\n");
    }

    fn write_module_decl_footer(&mut self, _name: &str) {
        self.write(");\n");
        if self.import_uvm {
            self.write("\n`ifndef SYNTHESIS\n");
            self.write("   import uvm_pkg::*;\n");
            self.write("`endif // SYNTHESIS\n");
        }
    }

    fn write_module_impl_footer(&mut self, name: &str) {
        self.write(&format!("\nendmodule : {name}\n"));
    }

    fn write_port_decl(&mut self, port: &PortInfo, prefix: Option<&String>, is_last: bool) {
        self.write("   ");
        match &port.dir {
            PortDir::In  => self.write("input  var "),
            PortDir::Out => self.write("output var "),
            PortDir::Modport(mp) => {
                let SignalKind::Custom((None,intf)) = &port.def.kind else {panic!("Expecting interface for port {:?}", port.def.kind)};
                self.write(&format!("{intf}.{mp} "))
            }
        }
        if port.dir.is_modport() {
            self.write(port.name());
        } else {
            self.add_signal_def(&port.def, prefix);
        }
        self.write(if is_last {"  "} else {", "});
        if !port.desc.is_empty() {
            self.write("// ");
            self.write(&port.desc);
        }
        self.write("\n");
    }

    fn write_signal_decl(&mut self, signal: &SignalDecl) {
        self.write("   ");
        self.add_signal_def(&signal.def, None);
        self.write(";");
        if !signal.desc.is_empty() {
            self.write(" // ");
            self.write(&signal.desc);
        }
        self.write("\n");
    }

    fn write_rif_decl(&mut self, comp: &CompInfo, single: bool, clk: &str, rst: &str) {
        let name = if single {"rif"} else {&comp.name};
        self.write(&format!("   rif_if#({}, {}) if_{name}({clk}, {rst});\n",
            comp.addr_width, comp.data_width
        ));
    }

    fn write_inst_header(&mut self, type_name: &str, inst_name: &str, params: &[(String, isize)]) {
        self.is_bridge = inst_name=="bridge";
        self.write(&format!("   {type_name}"));
        if !params.is_empty() {
            self.write("#(");
            let mut iter = params.iter().map(|(_,v)| v).peekable();
            while let Some(v) = iter.next() {
                self.write(&format!("{v}"));
                if iter.peek().is_some() {
                    self.write(", ");
                }
            }
            self.write(")");
        }
        self.write(&format!(" i_{inst_name} ("));
        if !self.is_bridge {
            self.write("\n");
        }
    }

    fn write_port_bind(&mut self, port_name: &str, signal_name: &str, is_last: bool) {
        let eol = if self.is_bridge {" "} else {"\n"};
        if port_name=="*" {
            self.write(".*")
        } else {
            if !self.is_bridge {
                self.write("      ");
            }
            self.write(&format!(".{port_name}({signal_name})"));
        }
        if is_last {
            self.write(eol);
            if !self.is_bridge {
                self.write("   ");
            }
            self.write(");\n\n");
        } else {
            self.write(",");
            self.write(eol);
        }
    }

    fn write_intf_bind(&mut self, name: &str, is_last: bool) {
        self.write(&format!("      .if_rif(if_{name})"));
        if is_last {
            self.write("\n   );\n\n");
        } else {
            self.write(",\n");
        }
    }

    fn write_assign(&mut self, lhs: ExprId, rhs: LogicExpr) {
        self.write("   assign ");
        self.add_expr_id(&lhs);
        self.write(" = ");
        let multiline =
            (rhs.has_vec() && rhs.nb() > 2 && !rhs.is_and()) ||
            (rhs.is_ite() && (rhs.nb() > 1 || !rhs.is_else_value()));
        let lvl = if multiline {self.write("\n"); 2} else {0};
        self.add_logic_expr(&rhs, lvl, false);
        self.write(";\n");
    }

    // Combinatorial process
    fn write_process_comb_header(&mut self, name: &str) {
        self.write(&format!("\n   always_comb begin : proc_{name}\n"));
    }
    fn write_process_comb_footer(&mut self, _name: &str) {
        self.write("   end\n\n");
    }

    fn write_match_header(&mut self, name: &str) {
        self.write(&format!("      case({name})\n"));
    }

    fn write_match_footer(&mut self) {
        self.write("      endcase\n");
    }

    fn write_match_case_header(&mut self, value: LogicExpr) {
        self.write("         ");
        self.add_logic_expr(&value, 0, false);
        self.write(" : begin\n");
    }

    fn write_match_case_footer(&mut self) {
        self.write("         end\n");
    }

    fn write_cond_if(&mut self, lvl: usize, cond: LogicExpr) {
        self.write(&" ".repeat(3*lvl));
        self.write("if(");
        self.add_logic_expr(&cond, 0, false);
        self.write(") begin\n");
    }

    fn write_cond_else(&mut self, lvl: usize, cond: Option<LogicExpr>) {
        self.write(&" ".repeat(3*lvl));
        self.write("else ");
        if let Some(cond) = cond {
            self.write_cond_if(0, cond);
        } else {
            self.write("begin\n");
        }
    }

    fn write_cond_end(&mut self, lvl: usize) {
        self.write(&" ".repeat(3*lvl));
        self.write("end\n");
    }

    fn write_generate_if(&mut self, cond: LogicExpr, name: String) {
        self.write("   if(");
        self.add_logic_expr(&cond, 0, false);
        self.write(") begin : ");
        self.write(&name);
        self.write("\n");
    }

    fn write_generate_else(&mut self, name: String) {
        self.write("   end else begin : ");
        self.write(&name);
        self.write("\n");
    }

    fn write_generate_end(&mut self, _name: String) {
        self.write("   end\n");
    }

    fn write_assign_comb(&mut self, lvl: usize, lhs: ExprId, rhs: LogicExpr) {
        self.write(&" ".repeat(3*lvl));
        self.add_expr_id(&lhs);
        self.write(" = ");
        self.add_logic_expr(&rhs, 0, false);
        self.write(";\n");
    }

    fn write_comment(&mut self, lvl: usize, txt: &str) {
        self.write(&" ".repeat(3*lvl));
        self.write("// ");
        self.write(txt);
        self.write("\n");
    }

    fn write_comment_box(&mut self, txt: &str) {
        self.write("\n//-----------------------------------------------------------------------------\n");
        for l in txt.split('\n') {
            self.write("// ");
            self.write(l);
            self.write("\n");
        }
        self.write("//-----------------------------------------------------------------------------\n");
    }

    fn write_assert(&mut self, name: &str, cond: LogicExpr, msg: String, is_uvm: bool) {
        self.write("\n`ifndef SYNTHESIS\n");
        self.write(&format!("   always @ (posedge {}) begin : proc_assert_{name}\n", self.sw_clk));
        self.write("      if(");
        self.add_logic_expr(&cond, 0, false);
        self.write(")\n         ");
        if is_uvm {
            self.write(&format!("uvm_report_error(\"RIF\", \"{msg}\", UVM_NONE);\n"));
        } else {
            self.write(&format!("$error(\"{msg}\")\n"));
        }
        self.write("   end\n");
        self.write("`endif // SYNTHESIS\n\n");
    }

    fn write_process_seq(&mut self, clk: &str, rst: &ResetDef, name: &str, signals: &[SignalInfo]) {
        // Check all signals clear/enable o see if all signals share a condition or not
        let mut signals_iter = signals.iter();
        let (clk_en, clr): (Option<&LogicExpr>, Option<&LogicExpr>) =
            if let Some(signal) = signals_iter.next() {
                (signal.enable.as_ref(), signal.clear.as_ref())
            } else {
                (None, None)
            };
        let mut clk_en_global = true;
        let mut clr_global = true;
        for signal in signals_iter {
            if clk_en_global {
                if let (Some(new), Some(base)) = (signal.enable.as_ref(), clk_en) {
                    if new != base {
                        clk_en_global = false;
                    }
                } else if clk_en.is_some() != signal.enable.is_some() {
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
        self.write(&format!(") begin : {name}\n      if("));
        // Reset
        if !rst.active_high {
            self.write("!");
        }
        self.write(&format!("{}) begin\n", rst.name));
        for signal in signals.iter() {
            self.write_signal_seq(&signal.name, &signal.reset, 3);
        }
        self.write("      end else ");
        // Optional Global clear
        // Maybe need to add an option to take the clear into account only if the enable is high
        if let Some(clr) = clr.filter(|_| clr_global) {
            self.write("if(");
            self.add_logic_expr(clr, 0, false);
            self.write(") begin\n");
            for signal in signals.iter() {
                self.write_signal_seq(&signal.name, &signal.reset, 3);
            }
            self.write("      end else ");
        }
        // Optional Global Enable
        if let Some(clk_en) = clk_en.filter(|_| clk_en_global) {
            self.write("if(");
            self.add_logic_expr(clk_en, 0, false);
            self.write(") ");
        }
        self.write("begin\n");
        // Set value
        let mut lvl = 3;
        for signal in signals.iter() {
            if let Some(clr) = signal.clear.as_ref().filter(|_| !clr_global) {
                lvl = 4;
                self.write("         if(");
                self.add_logic_expr(clr, 0, false);
                self.write(")\n");
                self.write_signal_seq(&signal.name, &signal.reset, lvl);
                self.write("         else");
            }
            if let Some(clk_en) = signal.enable.as_ref().filter(|_| !clk_en_global) {
                if lvl==3 {
                    self.write("         ");
                }
                self.write("if(");
                self.add_logic_expr(clk_en, 0, false);
                self.write(")");
                lvl = 4;
            }
            if lvl==4 {
                self.write("\n");
            }
            self.write_signal_seq(&signal.name, &signal.value, lvl);
            lvl = 3;
        }
        self.write("      end\n   end\n\n");
    }

}
