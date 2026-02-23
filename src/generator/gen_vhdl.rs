use std::collections::HashMap;

use crate::{
    cfg::{CfgRtl, RtlLimitCfg}, comp::{
        comp_inst::{RifInst, RifmuxInst},
        hw_info::{PortDir, PortInfo, RifIntfPorts, SignalDecl, SignalDef, SignalDim, SignalInfo, SignalKind}
    }, rifgen::{CastInfo, EnumEntry, ExprId, GenericRange, Interface, LogicExpr, ResetDef, order_dict::OrderDict}
};

use super::{
    gen_common::{CompInfo, GeneratorBase, GeneratorBaseSetting, GeneratorCore},
    trait_hw::GeneratorHw
};

use yarig_macro::add_gen_core;
#[add_gen_core("vhd")]
#[allow(dead_code)]
pub struct GeneratorVhdl {
    /// Number of pipe level for register access (default 1 on the read value)
    nb_pipe: u8,
    /// Generate constant for register address in the package
    const_reg: bool,
    /// Generate constant for field reset/position/width
    const_field: bool,
    /// Current enumerated type width
    enum_width : u8,
    /// Controls how field limits are used
    limit: RtlLimitCfg,
    /// True when current module instance is a bridge
    is_bridge : bool,
    /// Component interface
    intf : Interface,
    /// List of generic with their size (used for conversion in logical expression)
    generics : HashMap<String, u8>,
    /// List of outputs port (needed to add intermediate signals)
    outputs : OrderDict<String,String>,
    /// Internal state variable to track when output port have been handled
    outputs_locked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LogicExprKind {Basic, Bool, MathU, MathS}

impl GeneratorVhdl {

    pub fn new(setting: GeneratorBaseSetting, extra: CfgRtl) -> Self {
        let mut core = GeneratorCore::new(4,setting);
        // Override gen_inc if defined in the extra settings
        if let Some(gen_inc) = extra.gen_inc {
            core.setting.gen_inc = gen_inc;
        }
        let limit = RtlLimitCfg::new(extra.limit, extra.force_limit);
        GeneratorVhdl {
            core,
            enum_width: 0,
            nb_pipe: extra.nb_pipe.unwrap_or(1),
            const_reg: extra.const_reg.unwrap_or(false),
            const_field: extra.const_field.unwrap_or(false),
            limit,
            is_bridge: false,
            generics: HashMap::new(),
            outputs: OrderDict::new(),
            outputs_locked: false,
            intf: Interface::Default,
        }
    }

    fn write_signal_def(&mut self, def: &SignalDef, prefix: Option<&String> ) {
        let name = def.name.replace("__", "_");
        if let Some(prefix) = prefix {
            if let Some(base_name) = name.strip_prefix("rif_") {
                self.core.write(&format!("rif_{prefix}_{base_name}"));
            } else {
                self.core.write(&format!("{prefix}_{name}"));
            }
        } else {
            self.core.write(&name);
        }
        self.core.write(" : ");
        self.write_signal_kind(&def.kind, &def.dim);
    }

    fn signal_kind(&mut self, kind: &SignalKind, dim: &SignalDim) -> String {
        let mut kind_str = String::with_capacity(32);
        match kind {
            SignalKind::Signed(w) |
            SignalKind::Unsigned(w) => {
                if dim.is_null() {
                    kind_str.push_str("std_logic");
                    if *w > 1 {
                        kind_str.push_str(&format!("_vector({} downto 0)", w-1));
                    }
                } else {
                    match w {
                        1 => kind_str.push_str("t_sla"),
                        n => kind_str.push_str(&format!("t_slv_a{n}")),
                    }
                    kind_str.push_str(&format!("(0 to {dim:#})"));
                }
            },
            SignalKind::Custom((_,n)) => {
                kind_str.push_str(n);
                if dim.is_array() {
                    kind_str.push_str(&format!("_a(0 to {dim:#})"));
                }
            }
            SignalKind::Integer     => kind_str.push_str("integer "),
            SignalKind::Address     => kind_str.push_str(&format!("std_logic_vector({} downto 0)", self.addr_width()-1)),
            SignalKind::Data        => kind_str.push_str(&format!("std_logic_vector({} downto 0)", self.data_width()-1)),
        }
        kind_str
    }

    fn write_signal_kind(&mut self, kind: &SignalKind, dim: &SignalDim) {
        let k = self.signal_kind(kind, dim);
        self.write(&k);
    }

    fn write_signal_seq(&mut self, id: &ExprId, val: &LogicExpr, lvl: usize) {
        self.write(&" ".repeat(3*lvl));
        self.write_expr_id(id, LogicExprKind::Basic);
        self.write(" <= ");
        self.add_logic_expr(val, 0, LogicExprKind::Basic, false);
        self.write(";\n");
    }

    fn add_logic_expr(&mut self, expr: &LogicExpr, lvl: usize, kind: LogicExprKind, is_part: bool) {
        self.core.write(&" ".repeat(3*lvl));
        match expr {
            LogicExpr::Id(expr_id) => self.write_expr_id(expr_id, kind),
            LogicExpr::Cast(cast_info, expr) => {
                if let CastInfo::Custom(_, name) = cast_info {
                    self.core.write("to_");
                    self.core.write(name);
                    self.core.write("(");
                }
                self.add_logic_expr(expr, 0, kind, false);
                if cast_info.is_custom() {
                    self.core.write(")");
                }
            }
            LogicExpr::CastFrom(name, _, expr) => {
                self.core.write("from_");
                self.core.write(name);
                self.core.write("(");
                self.add_logic_expr(expr, 0, kind, false);
                self.core.write(")");
            }
            LogicExpr::ValueU(v, w) => {
                match w {
                    1 => self.core.write(&format!("'{v}'")),
                    2..7 => self.core.write(&format!("\"{v:0w$b}\"")),
                    _ => {
                        let n = (w+3)>>2;
                        self.core.write(&format!("{w}x\"{v:0n$x}\""))
                    },
                }
            }
            LogicExpr::ValueI(v, w) => {
                match w {
                    1..7 => self.core.write(&format!("\"{v:0w$b}\"")),
                    _ => {
                        let n = (w+3)>>2;
                        self.core.write(&format!("{w}x\"{v:0n$x}\""))
                    },
                }
            }
            LogicExpr::Not(logic_expr) if kind==LogicExprKind::Bool => {
                if let LogicExpr::Id(id) = &**logic_expr {
                    self.write_expr_id(id, LogicExprKind::Basic);
                    self.write(" = '0'");
                } else {
                    self.core.write("not ");
                    self.add_logic_expr(logic_expr, 0, kind, true);
                }
            }
            LogicExpr::Not(logic_expr) |
            LogicExpr::NotB(logic_expr) => {
                self.core.write("not ");
                self.add_logic_expr(logic_expr, 0, kind, true);
            }
            LogicExpr::Concat(exprs) => {
                let mut expr_iter = exprs.iter().peekable();
                while let Some(e) = expr_iter.next() {
                    self.add_logic_expr(e, 0, kind, false);
                    if expr_iter.peek().is_some() {
                        self.core.write(" & ");
                    }
                }
            }
            LogicExpr::Add(lhs, rhs) => self.add_two_expr(lhs, rhs, " + " , LogicExprKind::MathU, false),
            LogicExpr::Sub(lhs, rhs) => self.add_two_expr(lhs, rhs, " - " , LogicExprKind::MathU, false),
            LogicExpr::Eq(lhs, rhs)  => self.add_two_expr(lhs, rhs, " = " , LogicExprKind::Basic, false),
            LogicExpr::Neq(lhs, rhs) => self.add_two_expr(lhs, rhs, " /= ", LogicExprKind::Basic, false),
            LogicExpr::Gte(lhs, rhs) => self.add_two_expr(lhs, rhs, " >= ", LogicExprKind::MathU, false),
            LogicExpr::Gt (lhs, rhs) => self.add_two_expr(lhs, rhs, " > " , LogicExprKind::MathU, false),
            LogicExpr::Lte(lhs, rhs) => self.add_two_expr(lhs, rhs, " <= ", LogicExprKind::MathU, false),
            LogicExpr::Lt(lhs, rhs)  => self.add_two_expr(lhs, rhs, " < " , LogicExprKind::MathU, false),
            LogicExpr::Xor(lhs, rhs) => self.add_two_expr(lhs, rhs, " xor ", kind, is_part),
            LogicExpr::OrB(lhs, rhs) => self.add_two_expr(lhs, rhs, " or " , kind, is_part),
            LogicExpr::AndB(lhs, rhs) => self.add_two_expr(lhs, rhs, " and ", kind, is_part),
            LogicExpr::Or(vec)  => self.add_n_expr(vec, " or " , kind, is_part, lvl),
            LogicExpr::And(vec) => self.add_n_expr(vec, " and ", kind, is_part, lvl),
            LogicExpr::Ite(it_exprs, else_expr) => {
                let tab = if lvl>0 {format!("\n{}", " ".repeat(3*lvl))} else {" ".to_owned()};
                if is_part {self.core.write("(");}
                for it_expr in it_exprs {
                    self.add_logic_expr(&it_expr.1, 0, LogicExprKind::Basic, it_expr.1.nb() > 1 || it_expr.1.is_ite());
                    self.core.write(" when ");
                    self.add_logic_expr(&it_expr.0, 0, LogicExprKind::Bool, false);
                    self.core.write(" else");
                    self.core.write(&tab);
                }
                self.add_logic_expr(else_expr, 0, kind, else_expr.nb() > 1);
                if is_part {self.core.write(")");}
            }
        }
    }

    fn add_two_expr(&mut self, lhs: &LogicExpr, rhs: &LogicExpr, op: &str, kind: LogicExprKind, is_part: bool) {
        if is_part {self.core.write("(");}
        let kind = if kind==LogicExprKind::MathU && matches!(rhs, LogicExpr::ValueI(_,_)) { LogicExprKind::MathS} else {kind};
        self.add_logic_expr(lhs, 0, kind, true);
        self.core.write(op);
        if rhs.is_ite() {
            self.core.write("\n      ");
        }
        self.add_logic_expr(rhs, 0, kind, true);
        if is_part {self.core.write(")");}
    }

    fn add_n_expr(&mut self, exprs: &[LogicExpr], op: &str, kind: LogicExprKind, is_part: bool, lvl: usize) {
        let is_part = is_part && exprs.len() > 1;
        let eol = if !is_part && lvl > 0 && exprs.len() > 2 {
            format!("\n{}", " ".repeat(3*lvl))
        } else {
            "".to_owned()
        };
        if is_part {self.core.write("(");}
        let mut expr_iter = exprs.iter().peekable();
        while let Some(expr) = expr_iter.next() {
            self.add_logic_expr(expr, 0, kind, true);
            if expr_iter.peek().is_some() {
                self.core.write(op);
                self.core.write(&eol);
            }
        }
        if is_part {self.core.write(")");}
    }

    fn write_expr_id(&mut self, expr: &ExprId, kind: LogicExprKind) {
        let is_intf_field = expr.field.as_ref()
            .map(|f| Self::IF_RIF_FIELDS.contains(&f.trim()));
        let is_intf = expr.name.starts_with("if_") && is_intf_field==Some(true);
        let has_cast_gen = kind==LogicExprKind::MathU && self.generics.contains_key(&expr.name);
        if has_cast_gen {
            self.write("to_");
        }
        match kind {
            LogicExprKind::MathU => self.write("unsigned("),
            LogicExprKind::MathS => self.write("signed("),
            LogicExprKind::Bool  |
            LogicExprKind::Basic => {}
        }
        let mut name =
            if expr.name == "if_rif" {"reg".to_owned()}
            else if is_intf {expr.name.strip_prefix("if_").unwrap_or(&expr.name).to_owned()}
            else {expr.name.replace("__", "_")};
        if self.outputs.contains_key(&expr.name) {
            name.push_str("_l");
        }
        self.write(&name);
        if let Some(idx) = expr.idx {
            self.write(&format!("({idx})"));
        }
        if let Some(field) = &expr.field {
            self.write(
                if expr.name == "if_rif" || is_intf {"_"}
                else {"."}
            );
            self.write(field);
        }
        if let Some(r) = &expr.range {
            if r.is_bit() {
                self.write(&format!("({})",r.lsb));
            } else {
                self.write(&format!("({} downto {})", r.msb, r.lsb));
            }
        }
        if has_cast_gen {
            let w = self.generics.get(&expr.name).unwrap();
            self.write(&format!(",{w}"));
        }
        match kind {
            LogicExprKind::Bool => self.write(" = '1'"),
            LogicExprKind::MathU |
            LogicExprKind::MathS => {
                self.write(")");
            }
            LogicExprKind::Basic => {}
        }
    }

}

impl GeneratorHw for GeneratorVhdl {

    const SUPPORT_INTF      : bool = false;
    const SUPPORT_IMPL_BIND : bool = false;

    /// Flag when register constants (address/reset) should be generated
    fn has_const_reg(&self) -> bool {self.const_reg}

    /// Flag when field constants (mask, lsb, msb, reset) should be generated
    fn has_const_field(&self) -> bool {self.const_field}

    /// Flag when field constants (mask, lsb, msb, reset) should be generated
    fn limit_cfg(&self) -> RtlLimitCfg {
        self.limit
    }

    /// Write generic header for a file
    fn write_file_header(&mut self) {
        // Add header : TODO: configurable header
        self.write_comment(0, "File generated automatically: DO NOT EDIT.");
        self.write("\n");
    }

    /// Set the width of address/data for current RIF
    fn set_rif_info(&mut self, rif: &RifInst) {
        self.outputs.clear();
        self.outputs_locked = false;
        self.intf = rif.interface.clone();
    }

    /// Set the width of address/data for current RIF
    fn set_rifmux_info(&mut self, rifmux: &RifmuxInst) {
        self.intf = rifmux.interface.clone();
    }

    // Hooks for RIF package
    fn write_rif_pkg_header(&mut self, rif: &RifInst) {
        self.write_pkg_header(&rif.type_name);
        // Extract all arrays
        let mut names = Vec::new();
        let mut vec_a = Vec::new();
        for hw_reg in rif.hw_regs.values() {
            if names.contains(&&hw_reg.group) {continue;}
            if hw_reg.dim.val() > 0 {
                names.push(&hw_reg.group);
                if hw_reg.port.is_in() {
                    self.push_stash(3, &format!("   type t_{0}_hw_a is array (natural range <>) of t_{0}_hw;\n", hw_reg.group));
                }
                if hw_reg.port.is_out() {
                    self.push_stash(3, &format!("   type t_{0}_sw_a is array (natural range <>) of t_{0}_sw;\n", hw_reg.group));
                }
            }
            let hw_reg_def = rif.get_hw_reg(&hw_reg.group);
            for f in hw_reg_def.fields.iter().filter(|f| f.array > 0) {
                if vec_a.contains(&f.width) {continue;}
                vec_a.push(f.width);
            }
        }
        if !names.is_empty() {
            self.push_stash(3, "\n");
        }
        if !vec_a.is_empty() {
            vec_a.sort();
            let mut sizes = vec_a.iter().peekable();
            // Handle case of array of bit: the sorted vector ensure this is the first entry
            if let Some(1) = sizes.peek()  {
                self.push_stash(3, "   type t_sla is array (natural range <>) of std_logic;\n");
                sizes.next();
            }
            for s in sizes {
                self.push_stash(3, &format!("   type t_slv_a{s} is array (natural range <>) of std_logic_vector({} downto 0);\n", s-1));
            }
            self.push_stash(3, "\n");
        }
        if !rif.enum_defs.is_empty() {
            self.write("   attribute ENUM_ENCODING : string;\n");
        }
    }

    fn write_pkg_header(&mut self, name: &str) {
        self.write("\nlibrary ieee;\n");
        self.write("use ieee.std_logic_1164.all;\n\n");
        self.write(&format!("package {name}_pkg is\n\n"));
    }

    fn write_pkg_footer(&mut self, name: &str) {
        self.pop_stash(3);
        self.write(&format!("end package {name}_pkg;\n"));
        if !self.stash_is_empty(1) {
            self.write(&format!("\npackage body {name}_pkg is \n"));
            self.pop_stash(1);
            self.pop_stash(2);
            self.write(&format!("end package body {name}_pkg;\n"));

        }
    }

    fn write_const(&mut self, signal: &SignalDecl, value: LogicExpr) {
        self.write("   constant ");
        self.write_signal_def(&signal.def, None);
        self.core.write(" := ");
        match &value {
            LogicExpr::ValueU(v,w) => {
                if signal.name().ends_with("_W") {
                    self.core.write(&format!("{v:2}"))
                } else if signal.def.kind==SignalKind::Unsigned(1) {
                    self.core.write(&format!("'{v}'"))
                } else {
                    let n = (w+3)>>2;
                    self.core.write(&format!("{w}x\"{v:0n$x}\""))
                }
            },
            LogicExpr::ValueI(v,w) => {
                if matches!(signal.def.kind,SignalKind::Integer) {
                    self.core.write(&format!("{v}"))
                } else if signal.def.kind==SignalKind::Unsigned(1) {
                    self.core.write(&format!("'{v}'"))
                } else {
                    let n = (w+3)>>2;
                    self.core.write(&format!("{w}x\"{v:0n$x}\""))
                }
            }
            _ => self.add_logic_expr(&value, 0, LogicExprKind::Basic, false),
        }
        self.write(";\n");
    }

    // Enums declaration
    fn write_enum_header(&mut self, name: &str, width: u8) {
        self.write(&format!("   type {name} is ("));
        self.enum_width = width;
        // Start function body for encoding/decoding
        self.push_stash(1, &format!("   function to_{name}(val: std_logic_vector({} downto 0)) return {name} is begin\n", width -1));
        self.push_stash(1, "      case(val) is\n");
        self.push_stash(2, &format!("   function from_{name}(val: {name}) return std_logic_vector is begin\n"));
        self.push_stash(2, "      case(val) is\n");
    }

    fn write_enum_entry(&mut self, entry: &EnumEntry, is_last: bool) {
        self.write(&entry.name);
        self.write(if is_last {");\n"} else {", "});
        // Push encoding on stash
        let w = self.enum_width as usize;
        self.push_stash(0,&format!("{:0w$b}", entry.value));
        self.push_stash(0,if is_last {"\""} else {" "});
        // Push encoding/decoding case
        let case = if is_last {"others".to_owned()} else {format!("\"{:0w$b}\"", entry.value)};
        self.push_stash(1, &format!("         when {case} => return {};\n", entry.name));
        let case = if is_last {"others"} else {&entry.name};
        self.push_stash(2, &format!("         when {case} => return \"{:0w$b}\";\n", entry.value));
    }

    fn write_enum_footer(&mut self, name: &str, width: u8) {
        self.write(&format!("   attribute ENUM_ENCODING of {name} : type is \""));
        self.pop_stash(0);
        self.write(";\n");
        self.write(&format!("   function to_{name}(val: std_logic_vector({} downto 0)) return {name};\n", width -1));
        self.write(&format!("   function from_{name}(val: {name}) return std_logic_vector;\n"));
        // Close function declaration
        self.push_stash(1, "      end case;\n");
        self.push_stash(1, &format!("   end function to_{name};\n\n"));
        self.push_stash(2, "      end case;\n");
        self.push_stash(2, &format!("   end function from_{name};\n\n"));
    }

    // Structure declaration
    fn write_struct_header(&mut self, name: &str, _fields: &[SignalDecl]) {
        self.write(&format!("   type {name} is record\n"));
    }

    fn write_struct_field(&mut self, field: &SignalDecl, _is_last: bool) {
        self.write("      ");
        self.write_signal_def(&field.def, None);
        self.write("; -- ");
        self.write(&field.desc);
        self.write("\n");
    }

    fn write_struct_footer(&mut self, name: &str) {
        self.write(&format!("   end record {name};\n\n"));
    }

    // Module declaration
    fn write_module_decl_header(&mut self, name: &str) {
        self.write("library ieee;\n");
        self.write("use ieee.std_logic_1164.all;\n");
        self.write("use ieee.std_logic_misc.all;\n");
        self.write("use ieee.numeric_std.all;\n\n");
        self.write(&format!("use work.{name}_pkg.all;\n\n"));
        self.write(&format!("entity {name} is\n"));
    }

    fn write_module_generic_header(&mut self) {
        self.write("   generic (\n");
        self.generics.clear();
    }

    fn write_module_generic_decl(&mut self, name: &str, range: &GenericRange, is_last: bool) {
        let width = (u16::BITS - range.max.leading_zeros()).max(1) as u8;
        self.generics.insert(name.to_owned(), width);
        self.write(&format!("      {name} : integer range {} to {} := {}",
            range.min, range.max, range.default));
        let sep = if is_last {" "} else {","};
        self.write(sep);
        if let Some(desc) = &range.desc {
            self.write(&format!(" -- {desc}"));
        }
        self.write("\n");
        if is_last {
            self.write("   );\n")
        }
    }

    fn write_module_port_header(&mut self) {
        self.write("   port (\n");
    }

    fn write_module_decl_footer(&mut self, name: &str) {
        self.write("   );\n");
        self.write(&format!("end {name};\n\n"));
        self.write(&format!("architecture rtl of {name} is\n"));
    }

    fn write_module_impl_footer(&mut self, _name: &str) {
        self.write("\nend architecture;\n");
    }

    fn write_port_decl(&mut self, port: &PortInfo, prefix: Option<&String>, is_last: bool) {
        // Handle Rif Interface
        if port.kind().custom_name() == "rif_if" {
            let base_raw = port.name().strip_prefix("if_").unwrap_or(port.name());
            let base = base_raw.trim();
            let pad = " ".repeat(base_raw.len() - base.len());
            for p in RifIntfPorts::new(&Interface::Default, false).iter() {
                let n = p.name().strip_prefix("reg_").unwrap_or(p.name());
                let d = if p.dir.is_in() {"out"} else {"in "};
                let desc = p.desc.strip_prefix("Register ").unwrap_or(&p.desc);
                if self.is_bridge {
                    self.write("      ");
                }
                self.write(&format!("   {base}_{n}{pad} : {d} "));
                self.write_signal_kind(p.kind(), &SignalDim::Fixed(0));
                self.write(&format!("; -- {} {desc}\n", &port.desc));
            }
            return;
        }

        //
        self.write("   ");
        let name =
            if let Some(prefix) = prefix {
                if let Some(base_name) = port.name().strip_prefix("rif_") {
                    format!("rif_{prefix}_{base_name}")
                } else {
                    format!("{prefix}_{}",port.name())
                }
            } else {
                port.name().to_owned()
            };
        self.write(&name);
        match &port.dir {
            PortDir::In  => self.write(" : in  "),
            PortDir::Out => {
                self.write(" : out ");
                if !self.outputs_locked {
                    let typename = self.signal_kind(port.kind(), port.dim());
                    self.outputs.insert(name, typename);
                }
            }
            _ => {}
        }
        self.write_signal_kind(port.kind(), port.dim());
        self.write(if is_last {"  "} else {"; "});
        if !port.desc.is_empty() {
            self.write("-- ");
            self.write(&port.desc);
        }
        self.write("\n");
    }

    fn write_signal_decl(&mut self, signal: &SignalDecl) {
        self.write("   signal ");
        self.write_signal_def(&signal.def, None);
        self.write(";");
        if !signal.desc.is_empty() {
            self.write(" -- ");
            self.write(&signal.desc);
        }
        self.write("\n");
    }

    fn write_rif_decl(&mut self, _comp: &CompInfo, _single: bool, _clk: &str, _rst: &str) {
        let ports = RifIntfPorts::new(&Interface::Default, false);
        for port in ports.iter() {
            self.write("   signal ");
            self.write(port.name());
            self.write(" : ");
            self.write_signal_kind(port.kind(), port.dim());
            self.write(";\n");
        }
    }

    fn write_inst_header(&mut self, type_name: &str, inst_name: &str, params: &[(String, isize)]) {
        self.is_bridge = inst_name=="bridge";
        let type_name = type_name.replace("bridge_", "bridge_vhd_");
        self.write(&format!("   i_{inst_name} : {type_name}\n"));
        if !params.is_empty() {
            self.write("      generic map(\n");
            let mut iter = params.iter().peekable();
            while let Some((n,v)) = iter.next() {
                let sep = if iter.peek().is_some() {","} else {""};
                self.write(&format!("         {n} => {v}{sep}\n"));
            }
            self.write("      )\n");
        }
        self.write("      port map(\n");
    }

    fn write_port_bind(&mut self, port_name: &str, signal_name: &str, is_last: bool) {
        self.write(&format!("         {port_name} => {signal_name}"));
        if !is_last {
            self.write(",\n");
        } else {
            self.write("\n      );\n\n");
        }
    }

    fn write_intf_bind(&mut self, name: &str, is_last: bool) {
        let port_list = RifIntfPorts::new(&Interface::Default, false);
        let names = ["reg_err_addr_next", "reg_err_access_next"];
        let mut ports = port_list.iter().filter(|p| !names.contains(&p.name().trim())).peekable();
        while let Some(port) = ports.next() {
            let last = is_last && ports.peek().is_none();
            let sig_name =
                if name=="rif" {
                    port.name().to_owned()
                } else {
                    let f = port.name().strip_prefix("reg_").unwrap_or(port.name());
                    format!("reg_{name}_{f}")
                };
            self.write_port_bind(port.name(), &sig_name, last);
        }
    }

    fn write_assign(&mut self, lhs: ExprId, rhs: LogicExpr) {
        self.write("   ");
        self.write_expr_id(&lhs, LogicExprKind::Basic);
        self.write(" <= ");
        if rhs.has_comp() {
            self.write("'1' when ");
            self.add_logic_expr(&rhs, 0, LogicExprKind::Bool, false);
            self.write(" else '0'");
        } else {
            let multiline =
                (rhs.has_vec() && rhs.nb() > 2 && !rhs.is_and()) ||
                (rhs.is_ite() && (rhs.nb() > 1 || !rhs.is_else_value()));
            let lvl = if multiline {self.write("\n"); 2} else {0};
            self.add_logic_expr(&rhs, lvl, LogicExprKind::Basic, false);
        }
        self.write(";\n");
    }

    // Combinatorial process
    fn write_process_comb_header(&mut self, name: &str) {
        self.write(&format!("\n   proc_{name} : process(all) begin\n"));
    }
    fn write_process_comb_footer(&mut self, _name: &str) {
        self.write("   end process;\n\n");
    }

    fn write_match_header(&mut self, name: &str) {
        self.write(&format!("      case {name} is\n"));
    }

    fn write_match_footer(&mut self) {
        self.write("      end case;\n");
    }

    fn write_match_case_header(&mut self, value: LogicExpr) {
        self.write("         when ");
        if value.is_id("default") {
            self.write("others");
        } else {
            self.add_logic_expr(&value, 0, LogicExprKind::Basic, false);
        }
        self.write(" =>\n");
    }

    fn write_match_case_footer(&mut self) {}

    fn write_cond_if(&mut self, lvl: usize, cond: LogicExpr) {
        self.write(&" ".repeat(3*lvl));
        self.write("if ");
        self.add_logic_expr(&cond, 0, LogicExprKind::Bool, false);
        self.write(" then\n");
    }

    fn write_cond_else(&mut self, lvl: usize, cond: Option<LogicExpr>) {
        self.write(&" ".repeat(3*lvl));
        if let Some(cond) = cond {
            self.write("els");
            self.write_cond_if(0, cond);
        } else {
            self.write("else\n");
        }
    }

    fn write_cond_end(&mut self, lvl: usize) {
        self.write(&" ".repeat(3*lvl));
        self.write("end if;\n");
    }

    fn write_generate_if(&mut self, cond: LogicExpr, name: String) {
        self.write("   ");
        self.write(&name);
        self.write(": if(");
        self.add_logic_expr(&cond, 0, LogicExprKind::Bool, false);
        self.write(") generate");
        self.write("\n");
    }

    fn write_generate_else(&mut self, _name: String) {
        self.write("   else generate");
        self.write("\n");
    }

    fn write_generate_end(&mut self, name: String) {
        self.write("   end generate ");
        self.write(&name);
        self.write(";\n");
    }

    fn write_assign_comb(&mut self, lvl: usize, lhs: ExprId, rhs: LogicExpr) {
        self.write(&" ".repeat(3*lvl));
        self.write_expr_id(&lhs, LogicExprKind::Basic);
        self.write(" <= ");
        if rhs.has_comp() {
            self.write("'1' when ");
            self.add_logic_expr(&rhs, 0, LogicExprKind::Bool, false);
            self.write(" else '0'");
        } else {
            self.add_logic_expr(&rhs, 0, LogicExprKind::Basic, false);
        }
        self.write(";\n");
    }

    fn write_signal_decl_footer(&mut self, is_rif: bool) {
        // For RIF: need to intermediate signal for output
        if is_rif {
            self.write("\n   -- Intermediate signals for output port --\n");
            for (n,t) in self.outputs.items() {
                self.core.write(&format!("   signal {n}_l : {t};\n"));
                self.core.push_stash(0,&format!("   {n} <= {n}_l;\n"));
            }
        }
        // Optional bridge declaration
        if !self.intf.is_default() {
            self.write(&format!("\n   component bridge_vhd_{}_rif is\n", self.intf.name()));
            self.write("      generic (ADDR_W : natural := 16; DATA_W : natural := 32; ASSUME_WR_OK : boolean := false);\n");
            self.write("      port (\n");
            self.write("         clk            : in  std_logic;\n");
            self.write("         rst_n          : in  std_logic;\n");
            let mut ports = RifIntfPorts::new(&Interface::Default, false);
            let names = ["reg_err_addr_next", "reg_err_access_next"];
            for port in ports.iter_mut().filter(|p| !names.contains(&p.name().trim())) {
                if port.dir == PortDir::In {port.dir = PortDir::Out;}
                else {port.dir = PortDir::In;}
                self.write("      ");
                self.write_port_decl(port, None, false);
            }
            let intf_ports = RifIntfPorts::new(&self.intf, false);
            let mut ports = intf_ports.iter().peekable();
            while let Some(port) = ports.next() {
                self.write("      ");
                self.write_port_decl(port, None, ports.peek().is_none());
            }
            self.write("      );\n");
            self.write(&format!("   end component bridge_vhd_{}_rif;\n", self.intf.name()));
        }
        self.write("\nbegin\n\n");
        self.pop_stash(0);
    }

    fn write_comment(&mut self, lvl: usize, txt: &str) {
        self.write(&" ".repeat(3*lvl));
        self.write("-- ");
        self.write(txt);
        self.write("\n");
        if txt=="Register SW interface" {
            self.outputs_locked = true;
        }
    }

    fn write_comment_box(&mut self, txt: &str) {
        // Remove signals declaration box since VHDL has a section dedicated to signals declaration
        if txt=="Signals declaration" {return;}
        self.write("\n-------------------------------------------------------------------------------\n");
        for l in txt.split('\n') {
            self.write("-- ");
            self.write(l);
            self.write("\n");
        }
        self.write("-------------------------------------------------------------------------------\n");
    }

    fn write_assert(&mut self, _name: &str, cond: LogicExpr, msg: String, _is_uvm: bool) {
        self.write("-- pragma translate_off\n");
        self.write("assert (");
        self.add_logic_expr(&cond, 1, LogicExprKind::Bool, false);
        self.write(&format!(") report \"{msg}\" severity Error;\n"));
        self.write("-- pragma translate_on\n");
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
        self.write(&format!("\n   {name} : process({clk}"));
        if !rst.sync {
            self.write(&format!(", {}", rst.name));
        }
        self.write(") begin\n");
        // Reset
        let rst_pol = if rst.active_high {1} else {0};
        if !rst.sync {
            self.write(&format!("      if {} = '{rst_pol}' then\n", rst.name));
        } else {
            self.write(&format!("      if rising_edge({clk}) then\n"));
        }

        for signal in signals.iter() {
            self.write_signal_seq(&signal.name, &signal.reset, 3);
        }
        if !rst.sync {
            self.write(&format!("      elsif rising_edge({clk}) then\n"));
        }
        let base_lvl = if clk_en_global && clk_en.is_some() {4} else {3};
        let tab = " ".repeat(base_lvl*3);
        // Optional Global clear
        // Maybe need to add an option to take the clear into account only if the enable is high
        if let Some(clr) = clr.filter(|_| clr_global) {
            self.write("if ");
            self.add_logic_expr(clr, 0, LogicExprKind::Bool, false);
            self.write(" then\n");
            for signal in signals.iter() {
                self.write_signal_seq(&signal.name, &signal.reset, 3);
            }
            self.write("      end els");
        } else if base_lvl == 4 {
            self.write("      ");
        }
        // Optional Global Enable
        if let Some(clk_en) = clk_en.filter(|_| clk_en_global) {
            self.write("if ");
            self.add_logic_expr(clk_en, 0, LogicExprKind::Bool, false);
            self.write(" then\n");
        }
        // Set value
        let mut lvl = base_lvl;
        for signal in signals.iter() {
            if let Some(clr) = signal.clear.as_ref().filter(|_| !clr_global) {
                self.write(&tab);
                self.write("if ");
                self.add_logic_expr(clr, 0, LogicExprKind::Bool, false);
                self.write(" then");
                self.write_signal_seq(&signal.name, &signal.reset, lvl);
                self.write(&tab);
                self.write("els");
                lvl = base_lvl+1;
            }
            if let Some(clk_en) = signal.enable.as_ref().filter(|_| !clk_en_global) {
                if lvl==base_lvl {
                    self.write(&tab);
                }
                self.write("if ");
                self.add_logic_expr(clk_en, 0, LogicExprKind::Bool, false);
                self.write(" then");
                lvl = base_lvl+1;
            }
            if lvl==base_lvl+1 {
                self.write("\n");
            }
            self.write_signal_seq(&signal.name, &signal.value, lvl);
            if lvl==base_lvl+1 {
                self.write(&tab);
                self.write("end if;\n");
                lvl = base_lvl;
            }
        }
        if clk_en_global && clk_en.is_some() {
            self.write("          end if;\n");
        }
        self.write("      end if;\n");
        self.write("   end process;\n\n");
    }

}
