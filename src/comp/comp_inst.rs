use std::collections::{BTreeMap, HashMap};

use crate::{
    parser::{get_rif, parser_expr::ParamValues, RifGenSrc, RifGenTop},
    rifgen::{
        order_dict::{OrderDict, OrderedDictIterV}, Access, AddressKind, ClockingInfo, Description, EnumDef, EnumKind, ExternalKind, Field, FieldHwKind, FieldPos, FieldSwKind, Interface, InterruptRegKind, Limit, RegDef, RegDefOrIncl, RegIncludePath, RegInst, RegPulseKind, ResetVal, ResetValOverride, Rif, RifPage, RifType, Rifmux, RifmuxGroup, RifmuxTop, SuffixInfo, Visibility
    },
};

use super::{hw_info::PortList, reg_impl::{HwRegs, RegImpl, RegImplDict}};

#[derive(Clone, Debug)]
/// Instance of Rifmux
pub struct RifmuxInst {
    /// Type name
    pub type_name: String,
    /// Instance name
    pub inst_name: String,
    /// Address bus width
    pub addr_width: u8,
    /// Data bus width
    pub data_width: u8,
    /// Software interface clock definition
    pub sw_clocking: ClockingInfo,
    /// Top description
    pub description: Description,
    /// Hardware Interface
    pub interface: Interface,
    /// List of component instance
    pub components: Vec<CompInst>,
    /// List of component group
    pub groups: Vec<RifmuxGroupInst>,
    /// Optional top to instantiate rifmux and all referenced RIFs
    pub top: Option<RifmuxTop>,
}

#[derive(Clone, Debug)]
/// Instance of External Rif (i.e. basic memory space)
pub struct RifExt {
    pub inst_name: String,
    pub addr_width: u8,
    pub description: Description,
}

#[derive(Clone, Debug)]
/// Component: Rifmux, Rif or External Rif
pub enum Comp {
    Rifmux(RifmuxInst),
    Rif(RifInst),
    External(RifExt),
}

#[derive(Clone, Debug)]
/// Component instance: a component with an address and an optional group name
pub struct CompInst {
    pub inst: Comp,
    pub addr: u64,
    pub group: String,
}

impl CompInst {
    /// Create a RifMux component
    pub fn new_mux(inst: RifmuxInst, addr: u64, group: String) -> Self {
        CompInst { inst: Comp::Rifmux(inst), addr, group}
    }

    /// Create a Rif component
    pub fn new_rif(inst: RifInst, addr: u64, group: String) -> Self {
        CompInst { inst: Comp::Rif(inst), addr, group}
    }

    /// Create an external component
    pub fn new_ext(inst: RifExt, addr: u64, group: String) -> Self {
        CompInst { inst: Comp::External(inst), addr, group}
    }

    pub fn get_name(&self) -> &str {
        self.inst.get_name()
    }

    pub fn get_type(&self) -> &str {
        self.inst.get_type()
    }

    pub fn get_addr_width(&self) -> u8 {
        self.inst.get_addr_width()
    }

    pub fn get_desc_short(&self) -> &str {
        self.inst.get_desc_short()
    }

    pub fn full_addr(&self, groups: &[RifmuxGroupInst]) -> u64 {
        let mut addr = self.addr;
        if !self.group.is_empty() {
            let group_offset = if let Some(group) = groups.iter().find(|x| x.name==self.group) {group.addr} else {0_u64};
            addr += group_offset;
        }
        addr
    }

    pub fn is_external(&self) -> bool {
        matches!(self.inst, Comp::External(_))
    }

    pub fn get_rif(&self) -> Option<&RifInst> {
        match &self.inst {
            Comp::Rif(rif) => Some(rif),
            _ => None
        }
    }

}

/// Helper struct to process an instance address when giving an offset and an AddressKind (Absolute/relative)
struct InstAddr{
    base: i64,
    incr: u64
}

impl InstAddr {

    pub fn new(incr: u8) -> Self {
        InstAddr{base: 0 - incr as i64, incr: incr as u64}
    }

    pub fn updt(&mut self, offset: u64, kind: AddressKind) -> u64 {
        match kind {
            AddressKind::Absolute => {
                self.base = offset as i64;
                self.base as u64
            }
            AddressKind::Relative => {
                offset.max(self.incr) + (self.base as u64)
            }
            AddressKind::RelativeSet => {
                self.base += offset.max(self.incr) as i64;
                self.base as u64
            }
        }
    }

    pub fn incr(&mut self) -> u64 {
        self.base += self.incr as i64;
        self.base as u64
    }

    pub fn decr(&mut self) -> u64 {
        self.base -= self.incr as i64;
        self.base as u64
    }
}

/// Group instances with a common offset under a common name prefix
#[derive(Clone, Debug)]
pub struct RifmuxGroupInst {
    /// Name of the RIF instance
    pub name: String,
    /// Base address of the group
    pub addr: u64,
    /// Description
    pub description: Description,
}

impl RifmuxGroupInst {

    pub fn new(name: String, addr: u64, description: Description) -> Self {
        RifmuxGroupInst { name, addr, description }
    }

    pub fn from(groups: &[RifmuxGroup], params: &ParamValues) -> Vec<Self> {
        let mut v = Vec::with_capacity(groups.len());
        let mut addr_inst = InstAddr::new(0);
        for g in groups.iter() {
            let addr = addr_inst.updt(g.addr.value(params), g.addr_kind);
            v.push(RifmuxGroupInst::new(g.name.to_owned(), addr, g.description.clone()));
        }
        v
    }

}

#[derive(Clone, Debug)]
/// RIFs sources definition, parameter setting and dictionnary of partial fiels used to build RIF instance and implementation
pub struct RifsInfo<'a> {
    /// RIFs definition
    pub rifs: &'a HashMap<String, Rif>,
    /// Parameter values
    pub params: ParamValues,
    /// Partial field dictionnary
    pub partials: PartialFieldDict,
}

impl<'a> RifsInfo<'a>  {
    pub fn new(rifs: &'a HashMap<String, Rif>, params: ParamValues) -> Self {
        RifsInfo {
            rifs,
            params,
            partials: PartialFieldDict::new()
        }
    }
}

#[derive(Clone, Debug)]
pub struct RifInst {
    /// Instance name
    pub inst_name: String,
    /// Type name
    pub type_name: String,
    /// Address bus width
    pub addr_width: u8,
    /// Data bus width
    pub data_width: u8,
    /// Instance description
    pub description: Description,
    /// Type description
    pub base_description: Description,
    /// Enum definition
    pub enum_defs: Vec<EnumDef>,
    /// Register pages
    pub pages: Vec<RifPageInst>,
    /// Register structure definition (hardware implementation)
    pub reg_impl_defs: RegImplDict,
    /// List of register group instance
    pub hw_regs: HwRegs,
    /// List of all ports (clocks, resets, enable, clear, ...)
    pub ports: PortList,
    /// Software interface
    pub interface: Interface,
    /// Suffix information
    pub suffix: Option<SuffixInfo>,
    /// Software interface clock definition
    pub sw_clocking: ClockingInfo,
    /// Hardware interface clock definition
    pub hw_clocking: Vec<ClockingInfo>,
    /// Extra Custom information
    pub info: BTreeMap<String,String>,
    /// Parameter values
    pub params: ParamValues,
}

impl RifInst {
    pub fn new(name: &str, rif: &Rif, top_params: &ParamValues, rifs: &HashMap<String, Rif>, description: Description, suffix: Option<SuffixInfo>) -> Result<Self, String> {
        let addr_incr = rif.data_width >> 3; // Address align in byte
        let mut params = top_params.clone();
        params.compile(rif.parameters.items())?;
        // if !params.is_empty() {println!("{} : {}", rif.name, params);}
        let mut rifs_info = RifsInfo::new(rifs, params);
        let mut enum_defs = rif.enum_defs.clone();
        // Collect all register instantiated in a page
        let mut pages : Vec<RifPageInst> = Vec::with_capacity(rif.pages.len());
        for page in rif.pages.iter() {
            // Check for included rif and update the enum defs
            for inc in page.registers.iter().filter_map(RegDefOrIncl::get_inc) {
                let path = RegIncludePath::new(inc)?;
                let Some(inc_rif) = get_rif(rifs, path.rif) else {
                    return Err(format!("Unable to find {} in RIF definitions ({:?})", path.rif , rifs.keys()));
                };
                if inc_rif.data_width > rif.data_width {
                    return Err(format!("Included RIF {} uses larger register ({}) than {} ({}) !",
                        path.rif , inc_rif.data_width, rif.name, rif.data_width));
                }
                for e in &inc_rif.enum_defs {
                    // If the type name is documentation or defined in an external package
                    if e.name.contains(':') {
                        enum_defs.push(e.clone());
                    } else {
                        let mut enum_def = EnumDef::new(format!("{}_pkg::{}", inc_rif.name, e.name), e.description.clone());
                        enum_def.values = e.values.clone();
                        enum_defs.push(enum_def);
                    }
                }
            }
            // Create page instance
            let inst = RifPageInst::new(&mut rifs_info, page, addr_incr)?;
            pages.push(inst);
        }
        // Create the register implementation (for Hardware definition)
        let reg_impl_defs = RegImplDict::build(rif, &rifs_info)?;
        let hw_regs = HwRegs::build(&pages, &reg_impl_defs, &rifs_info)?;
        // Create hardware port list associated with the RIF instance
        let ports = PortList::new(rif, &pages, &reg_impl_defs, &hw_regs, &suffix);
        // Copy all relevant information in the RIF instance
        Ok(RifInst {
            inst_name: name.to_owned(),
            type_name: rif.name.to_owned(),
            addr_width: rif.addr_width,
            data_width: rif.data_width,
            enum_defs,
            description: if description.is_empty() {rif.description.clone()} else {description},
            base_description: rif.description.clone(),
            pages,
            reg_impl_defs, hw_regs,
            ports,
            interface: rif.interface.clone(),
            suffix,
            sw_clocking: rif.sw_clocking.clone(),
            hw_clocking: rif.hw_clocking.clone(),
            info: rif.info.clone(),
            params: rifs_info.params
        })
    }

    /// Retrieve a hardware register implementation
    /// Suppose always exists except for coding error
    pub fn get_hw_reg(&self, name: &String) -> &RegImpl {
        self.reg_impl_defs.get(name)
            .unwrap_or_else(|| panic!("{name} should be defined in the hardware registers !"))
    }

    /// Retrieve an enum definition
    pub fn get_enum_def(&self, name: &str) -> Result<&EnumDef, String>{
        if let Some(ed) = self.enum_defs.iter().find(|e| e.name == name) {
            Ok(ed)
        } else {
            Err(format!("Unable to find enum {name}! Known enums are {:?}",
                self.enum_defs.iter().map(|e| &e.name).collect::<Vec<&String>>()))
        }
    }

    /// Name with optional prefix taken into account
    pub fn name(&self, is_pkg: bool) -> String {
        if let Some(suffix) = &self.suffix {
            if is_pkg && !suffix.pkg  {
                self.type_name.clone()
            } else if suffix.alt_pos && self.type_name.ends_with("_rif") {
                format!("{}_{}_rif", &self.type_name[..(self.type_name.len()-4)], suffix.name)
            } else {
                format!("{}_{}", self.type_name, suffix.name)
            }
        } else {
            self.type_name.clone()
        }
    }


}

#[derive(Clone, Debug)]
pub struct RifPageInst {
    pub name: String,
    pub regs: Vec<RifRegInst>,
    pub addr: u64,
    pub external: Option<u8>,
    pub description: Description,
    reg_lut: OrderDict<String,Vec<usize>>
}

impl RifPageInst {
    pub fn new(
        rifs: &mut RifsInfo,
        page: &RifPage,
        addr_incr: u8,
    ) -> Result<Self, String> {
        let mut p = RifPageInst {
            name: page.name.to_owned(),
            addr: page.addr,
            description: page.description.clone(),
            regs: Vec::new(),
            reg_lut: OrderDict::new(),
            external: if page.external {Some(page.addr_width)} else {None}
        };
        // Automatic instance: create one register from each definition
        // and check for any override in the instances vector
        if page.inst_auto {
            Self::reg_auto_inst(&mut p, rifs, page, addr_incr)?;
        }
        // Manual instance: create one register by instance
        // and retrieve all needed information from the definition
        else {
            let mut inst_addr = InstAddr::new(addr_incr);
            for reg in page.instances.iter() {
                if let Some(ovr) = reg.reg_override.get(&None) {
                    if !ovr.optional.is_empty() && ovr.optional.eval(&rifs.params)? == 0 {
                        continue;
                    }
                }
                if let Some((def,intr_reg_kind,idx)) = page.find_regdef(&reg.type_name,rifs.rifs ) {
                    let addr = inst_addr.updt(reg.addr, reg.addr_kind);
                    // println!("Reg {} with {:?}({:04x}) -> {:04x}", reg.inst_name, reg.addr_kind, reg.addr, addr);

                    let nb = reg.array.eval(&rifs.params)? as u16;
                    // For array create one instance per element with the array information
                    if nb > 1 {
                        inst_addr.decr(); // Pre-decrement because address will be incremented for each array element
                        // println!("Array of size {nb} found for {} (Manual)", reg.inst_name);
                        for i in 0..nb {
                            p.add_reg(RifRegInst::new(def, inst_addr.incr(), Some(reg), RegInstArgs::Arr(ArrayIdx::Inst(i, nb)), rifs)?);
                        }
                    }
                    // For non-array simply add the register with the optional interrupt information
                    else {
                        p.add_reg(RifRegInst::new(def, addr,  Some(reg), RegInstArgs::Intr(intr_reg_kind,idx, false), rifs)?);
                    }
                } else {
                    return Err(format!("Missing definition for {}", reg.type_name));
                }
            }
        }
        Ok(p)
    }

    pub fn reg_auto_inst(&mut self,
        rifs: &mut RifsInfo,
        page: &RifPage,
        addr_incr: u8,
    ) -> Result<(), String> {
        let mut addr = 0_u64;
        for r in page.registers.iter() {
            match r {
                RegDefOrIncl::Include(inc) => {
                    let path = RegIncludePath::new(inc)?;
                    let Some(rif) = get_rif(rifs.rifs, path.rif) else {
                        return Err(format!("Unable to find {} in RIF definitions ({:?})", path.rif , rifs.rifs.keys()));
                    };
                    let Some(inc_page) = rif.pages.iter().find(|x| x.name == path.page) else {
                        return Err(format!("Unable to find page {} in {})", path.page, path.rif));
                    };
                    //
                    if path.reg == "*" {
                        self.reg_auto_inst(rifs, inc_page, addr_incr)?;
                    } else {
                        return Err(format!("Single register include not supported yet : {inc}"));
                    }
                }
                RegDefOrIncl::Def(d) => {
                    if d.ignored(&rifs.params) {
                        continue;
                    }
                    // Look for override settings inside the page instance
                    let inst = page.find_reg_inst(&d.name);
                    // For interrupt register create one register instance per optional property (enable/mask/pending)
                    if !d.interrupt.is_empty() {
                        for (idx, info) in d.interrupt.iter().enumerate() {
                            self.add_reg(RifRegInst::new(d, addr, inst, RegInstArgs::Intr(InterruptRegKind::Base, idx, true), rifs)?);
                            addr += addr_incr as u64;
                            if info.enable.is_some() {
                                self.add_reg(RifRegInst::new(d, addr, inst, RegInstArgs::Intr(InterruptRegKind::Enable, idx, true), rifs)?);
                                addr += addr_incr as u64;
                            }
                            if info.mask.is_some() {
                                self.add_reg(RifRegInst::new(d, addr, inst, RegInstArgs::Intr(InterruptRegKind::Mask, idx, true), rifs)?);
                                addr += addr_incr as u64;
                            }
                            if info.pending {
                                self.add_reg(RifRegInst::new(d, addr, inst, RegInstArgs::Intr(InterruptRegKind::Pending, idx, true), rifs)?);
                                addr += addr_incr as u64;
                            }
                        }
                    } else {
                        let nb = d.array.value(&rifs.params) as u16;
                        if nb > 1 {
                            // println!("Array of size {nb} found for {} (Auto)", d.name);
                            for i in 0..nb {
                                self.add_reg(RifRegInst::new(d, addr, inst, RegInstArgs::Arr(ArrayIdx::Def(i, nb)), rifs)?);
                                addr += addr_incr as u64;
                            }
                        } else {
                            self.add_reg(RifRegInst::new(d, addr, inst, RegInstArgs::Basic, rifs)?);
                            addr += addr_incr as u64;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Add a new register instance
    /// Fill a look-up table with indexes of registers grouped by type
    pub fn add_reg(&mut self, inst: Option<RifRegInst>) {
        if let Some(inst) = inst {
            self.reg_lut.entry(&inst.reg_type).push(self.regs.len());
            self.regs.push(inst);
        }
    }

    /// Iterator on register type
    pub fn iter_reg_type(&self) -> RegInstTypeIter {
        RegInstTypeIter {
            regs: &self.regs,
            lut_iter: self.reg_lut.values(),
        }
    }

    /// True when page is external
    pub fn is_external(&self) -> bool {
        self.external.is_some()
    }
}

//-----------------------------------------------------------------------------
// Implement iterator on value only
pub struct RegInstTypeIter<'a> {
    regs: &'a Vec<RifRegInst>,
    lut_iter: OrderedDictIterV<'a,Vec<usize>>,
}

impl<'a> Iterator for RegInstTypeIter<'a> {
    type Item = &'a RifRegInst;

    fn next(&mut self) -> Option<Self::Item> {
        let v = self.lut_iter.next()?;
        Some(&self.regs[v[0]])
    }
}

//-----------------------------------------------------------------------------

/// Pair of u16 giving index over dimension
/// The enum variant allows to make the difference between an array at the register definition level or the instance level
#[derive(Clone, Copy, Debug)]
pub enum ArrayIdx{
    Def(u16,u16),
    Inst(u16,u16),
}

impl ArrayIdx {

    pub fn idx(&self) -> u16 {
        match self {
            ArrayIdx::Def(idx, _) => *idx,
            ArrayIdx::Inst(idx, _) => *idx,
        }
    }

    pub fn dim(&self) -> u16 {
        match self {
            ArrayIdx::Def(_, dim) => *dim,
            ArrayIdx::Inst(_, dim) => *dim,
        }
    }

    pub fn is_def(&self) -> bool {
        matches!(self,ArrayIdx::Def(_,_))
    }

    pub fn is_inst(&self) -> bool {
        matches!(self,ArrayIdx::Inst(_,_))
    }
}

impl std::fmt::Display for ArrayIdx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArrayIdx::Def(idx, dim) => write!(f, "{idx}/{dim} (Def)"),
            ArrayIdx::Inst(idx, dim) => write!(f, "{idx}/{dim} (Inst)"),
        }
    }
}

pub enum RegInstArgs {
    Intr(InterruptRegKind,usize,bool),
    Arr(ArrayIdx),
    Basic
}

/// Register instance
#[derive(Clone, Debug)]
pub struct RifRegInst {
    pub reg_type: String,
    pub reg_name: String,
    pub group_name: String,
    pub group_type: String,
    pub external: ExternalKind,
    pub pulse: Vec<RegPulseKind>,
    pub description: Description,
    pub base_description: Description,
    pub intr_info: (InterruptRegKind, String),
    pub addr: u64,
    pub reset: u128,
    pub fields: Vec<RifFieldInst>,
    pub sw_access: Access,
    pub hw_access: Access,
    pub array: ArrayIdx,
    pub group_idx: usize,
    pub visibility: Visibility,
}

impl RifRegInst {
    pub fn new(
        def: &RegDef,
        addr: u64,
        inst: Option<&RegInst>,
        args: RegInstArgs,
        rifs: &mut RifsInfo,
    ) -> Result<Option<Self>, String> {
        let inst_name: String;
        let group_name: String;

        if let Some(i) = inst {
            inst_name = i.inst_name.to_owned();
            group_name =
                if i.group_name.is_empty() {def.get_group_name().to_owned()}
                else {i.group_name.to_owned()};
        } else {
            inst_name = def.name.to_owned();
            group_name = def.get_group_name().to_owned();
        }
        let mut sw_access = Access::NA;
        let mut hw_access = Access::NA;
        for f in def.fields.iter() {
            sw_access.updt((&f.sw_kind).into());
            hw_access.updt(f.hw_acc);
        }
        let intr_info;
        let reg_name;
        if let RegInstArgs::Intr(kind,idx,is_auto) = args {
            let intr_name = if idx > 0 && !def.interrupt[idx].name.is_empty() {format!("_{}",def.interrupt[idx].name)} else {"".to_owned()};
            let alt_name = if idx > 0 && is_auto {&intr_name} else {""};
            let suffix = if is_auto {kind.get_suffix()} else {""};
            reg_name = format!("{inst_name}{alt_name}{suffix}",);
            intr_info = (kind, intr_name.to_owned());
        } else {
            reg_name = inst_name;
            intr_info = (InterruptRegKind::None,"".to_owned());
        };
        let description = if let RegInstArgs::Arr(idx) = args {
            def.description.interpolate(idx.idx())
        } else {
            def.description.to_owned()
        };
        let mut r = RifRegInst {
            reg_type: def.name.to_owned(),
            reg_name,
            sw_access,
            hw_access,
            pulse: def.pulse.clone(),
            external: def.external,
            group_type: def.get_group_name().to_owned(),
            group_name,
            description,
            base_description: def.description.to_owned(),
            intr_info,
            addr,
            reset: 0,
            group_idx: 0,
            fields: Vec::new(),
            array : if let RegInstArgs::Arr(idx) = args {idx} else {ArrayIdx::Def(0,0)},
            visibility: def.visibility,
        };
        let mut next_lsb = 0;
        for f in def.fields.iter() {
            let array_size = f.array.value(&rifs.params) as u16;
            let nb = array_size.max(1);
            let offset = r.array.idx() * nb + f.partial.1;
            for i in 0..nb {
                let arr_idx =
                    if array_size > 0 {
                        if r.array.dim()>0 && r.array.is_def() {Some(ArrayIdx::Def(i,offset))}
                        else {Some(ArrayIdx::Inst(i,offset))}
                    } else { None };
                let fi = RifFieldInst::new(f, &mut next_lsb, &rifs.params, arr_idx);
                r.fields.push(fi);
            }
        }
        // Force Description & Reset value in case of derived interrupt register
        if let RegInstArgs::Intr(kind,idx,_) = args {
            let info = if kind.is_derived() {def.interrupt[idx].get_rst_desc(kind)} else {None};
            if let Some(info) = info {
                if !info.1.is_empty() {
                    r.base_description = info.1.clone();
                    r.description = info.1.interpolate(idx as u16);
                }
                if kind.is_pending() {
                    r.sw_access = Access::RO;
                }
                r.hw_access = Access::NA;
                r.reset = info.0.to_u128(128);
                for f in r.fields.iter_mut() {
                    let val : u128 = (r.reset >> f.lsb) & ((1<<f.width)-1);
                    f.reset = if f.is_signed() {
                        let val_signed = val as i128 - if val >= (1<<(f.width-1)) {(1<<f.width) as i128} else {0};
                        ResetVal::Signed(val_signed)
                    } else {
                        ResetVal::Unsigned(val)
                    };
                    if kind.is_pending() {
                        f.sw_kind = FieldSwKind::ReadOnly;
                    }
                    f.hw_kind.clear();
                }
            }
        }

        // Handle override settings
        if let Some(inst) = inst {
            let idx = if r.array.dim() > 1 {Some(r.array.idx())} else {None};
            if let Some(ovr) = inst.reg_override.get(&idx) {
                // Register override: Description
                if let Some(desc) = &ovr.description {
                    r.description = if let Some(i) = idx {
                        desc.interpolate(i)
                    } else {
                        desc.clone()
                    };
                }
                if !ovr.optional.is_empty() {
                    let optional = ovr.optional.eval(&rifs.params)?;
                    if optional == 0 {
                        return Ok(None);
                    }
                }
                if let Some(v) = ovr.visibility {
                    r.visibility = v;
                }
                if let Some(hw_access) = ovr.hw_acc {
                    r.hw_access = hw_access;
                }
                // Field override : Description, optional visbility, reset, limit, info
                for (k,ovr_f) in ovr.fields.iter() {
                    let Some(reg_field) = r.fields.iter_mut()
                        .find(|field| field.name == *k || field.name() == *k) else {
                            return Err(format!("Field {k} must exist in {}",r.reg_type));
                        };
                    if let Some(desc) = &ovr_f.description {
                        reg_field.description = desc.clone();//interpolate(idx);
                    }
                    if let Some(visibility) = ovr_f.visibility {
                        reg_field.visibility = visibility;
                    }
                    if !ovr_f.optional.is_empty() {
                        let optional = ovr_f.optional.eval(&rifs.params)?;
                        if optional == 0 {
                            reg_field.visibility = Visibility::Disabled;
                        }
                    }
                    match &ovr_f.reset {
                        ResetValOverride::Reset(reset_val) => reg_field.reset = reset_val.clone(),
                        ResetValOverride::Disable(reset_val) => {
                            reg_field.reset = reset_val.clone();
                            reg_field.visibility = Visibility::Disabled;
                        },
                        // Nothing
                        ResetValOverride::None => {},
                    }
                    if let Some(limit) = &ovr_f.limit {
                        reg_field.limit = limit.clone();
                    }
                }
            }
        }

        // get the register value once all override were applied
        for f in r.fields.iter() {
            let reset = f.reset.to_u128(f.width);
            if f.partial.0.is_some() {
                r.group_idx = rifs.partials.push(&r.group_type, &r.group_name, PartialFieldInfo::new(f));
            }
            r.reset |= reset << f.lsb;
        }

        // Sort vector in ascending order of position
        r.fields.sort_unstable_by_key(|f| f.lsb);
        Ok(Some(r))
    }

    /// Flag when a register uses an external implementation
    pub fn is_external(&self) -> bool {
        self.external.is_rw()
    }

    pub fn is_intr(&self) -> bool {
        self.intr_info.0.is_base()
    }

    pub fn is_intr_derived(&self) -> bool {
        self.intr_info.0.is_derived()
    }

    /// Flag when a register needs to implement a sequential process
    pub fn has_proc(&self) -> bool {
        for field in self.fields.iter() {
            if field.is_sw_write() || field.is_hw_write() {
                return true;
            }
        }
        false
    }

    /// Return the register type with interrupt suffix when register is a derived interrupt
    pub fn expanded_type_name(&self) -> String {
        if self.intr_info.0.is_derived() {
            format!("{}{}{}", self.reg_type, self.intr_info.1, self.intr_info.0.get_suffix())
        } else {
            self.reg_type.to_owned()
        }
    }

    /// Return a name with index array concatenated
    pub fn name(&self) -> String {
        if self.array.dim() > 0 {
            format!("{}{}",self.reg_name, self.array.idx())
        } else {
            self.reg_name.to_owned()
        }
    }

    /// Return register name with definition index array in bracket
    pub fn name_i(&self) -> String {
        if self.array.dim() > 0 {
            format!("{}[{}]",self.reg_name, self.array.idx())
        } else {
            self.reg_name.to_owned()
        }
    }

    /// Return group name with with optional interrupt base suffix
    pub fn group_name(&self) -> String {
        format!("{}{}", self.group_name, self.intr_info.1)
    }

    /// Return a group name with instance index array in bracket
    pub fn group_name_i(&self) -> String {
        if let ArrayIdx::Inst(idx,_) = self.array {
            format!("{}[{idx}]", self.group_name)
        } else {
            self.group_name.to_owned()
        }
    }
}

#[derive(Clone, Debug)]
pub struct RifFieldInst {
    pub name: String,
    pub lsb: u8,
    pub width: u8,
    pub base_description: Description,
    pub description: Description,
    pub reset: ResetVal,
    pub sw_kind: FieldSwKind,
    pub hw_kind: Vec<FieldHwKind>,
    pub visibility: Visibility,
    pub enum_kind: EnumKind,
    pub partial: (Option<u16>, u16),
    pub array: ArrayIdx,
    pub limit: Limit,
}

impl RifFieldInst {

    pub fn new(
        field: &Field,
        next_lsb: &mut u8,
        params: &ParamValues,
        array: Option<ArrayIdx>,
    ) -> Self {
        let (mut lsb, width) = match &field.pos {
            FieldPos::MsbLsb((m, l)) => (l.value(params), m.value(params) - l.value(params) + 1),
            FieldPos::LsbSize((l, w)) => (l.value(params), w.value(params)),
            FieldPos::Size(w) => (*next_lsb, w.value(params)),
        };
        let mut reset = field.reset.first().unwrap_or_default().clone();
        let idx : ArrayIdx;
        let desc: Description;
        // For array, adjust the increment when needed
        if let Some(array) = array {
            let incr = if field.array_pos_incr < width {
                width
            } else {
                field.array_pos_incr
            };
            lsb += array.idx() as u8 * incr;

            // Get the reset value if enough value are provided
            // (otherwise simply repeat the one at indice 0)
            let rst_idx = array.idx() as usize
                        + if array.is_def() {array.dim() as usize} else {0};
            if field.reset.len() > rst_idx {
                reset = field.reset.get(rst_idx).unwrap().clone();
            }
            let i = array.dim() + array.idx();
            idx = ArrayIdx::Def(i,field.array.value(params).into());
            // println!("Field array: {array:?} | rst_idx={rst_idx}, idx={idx:?} | reset = {reset:?}", );
            desc = field.description.interpolate(i);
        } else {
            idx = ArrayIdx::Def(0,0);
            desc = field.description.to_owned();
        }
        if let ResetVal::Param(p) = reset {
            let v = params.get(&p).expect("Undefined parameter in reset value");
            reset = if *v < 0 {ResetVal::Signed(*v as i128)} else {ResetVal::Unsigned(*v as u128)};
        }
        //
        let mut hw_kind = field.hw_kind.to_owned();
        if let Some(kind) = field.get_auto_hw_kind(params) {
            hw_kind.push(kind);
        }
        //
        *next_lsb += width;
        RifFieldInst {
            name: field.name.to_owned(),
            base_description: desc.clone(),
            description: desc,
            reset,
            sw_kind: field.sw_kind.to_owned(),
            hw_kind,
            visibility: field.visibility,
            enum_kind: field.enum_kind.clone(),
            limit: field.limit.clone(),
            partial: field.partial,
            lsb,
            width,
            array: idx,
        }
    }

    /// Flag when a field is disabled
    pub fn is_disabled(&self) -> bool {
        self.visibility.is_disabled()
    }

    /// Flag when a field is disabled
    pub fn is_reserved(&self) -> bool {
        self.visibility.is_reserved()
    }

    /// Flag when the field can be written by software
    pub fn is_sw_write(&self) -> bool {
        self.sw_kind!=FieldSwKind::ReadOnly
    }

    /// Flag when the field can be written by hardware
    pub fn is_hw_write(&self) -> bool {
        if self.width > 1 {
            self.hw_kind.iter().any(|k| *k!=FieldHwKind::ReadOnly)
        } else {
            self.hw_kind.iter().any(|k| k.has_write_mod() || k.is_counter() || k.is_interrupt())
        }
        // !self.hw_kind.is_empty()
    }

    /// Flag when a field is signed
    pub fn is_signed(&self) -> bool {
        matches!(self.reset, ResetVal::Signed(_))
    }

    /// Flag when a field has a limit constraint
    pub fn has_limit(&self) -> bool {
        !self.limit.is_none()
    }

    #[allow(dead_code)]
    /// Flag when a field is a counter
    pub fn is_counter(&self)  -> bool {
        self.hw_kind.iter().any(|x| x.is_counter())
        // self.hw_kind.iter().find(|&x| x.is_counter()).is_some()
    }

    /// Flag when a field is a password
    pub fn is_password(&self) -> bool {
        self.sw_kind.is_password()
    }

    /// Flag field which has a Hardware Write Enable
    pub fn has_write_mod(&self) -> bool {
        self.hw_kind.iter().any(|x| x.has_write_mod())
    }

    /// Return the field position MSB
    pub fn msb(&self) -> u8 {
        self.lsb + self.width - 1
    }

    /// Return name with index in bracket if part of an array
    pub fn name(&self) -> String {
        if self.array.dim() > 0 {
            format!("{}[{}]", self.name, self.array.idx())
        } else {
            self.name.to_owned()
        }
    }

    /// Return name with index in bracket if part of an array
    pub fn name_flat(&self) -> String {
        if self.array.dim() > 0 {
            format!("{}{}", self.name, self.array.idx())
        } else {
            self.name.to_owned()
        }
    }
}

#[derive(Clone, Debug)]
/// Partial field info used to recover the complete field reset value
/// and check the whole field is properly defined
pub struct PartialFieldInfo {
    /// Field name
    pub name: String,
    /// Position in the whole field
    pub lsb: u16,
    /// Width of the partial field
    pub width: u16,
    /// Reset value
    pub reset: u128
}

impl PartialFieldInfo {
    pub fn new(inst: &RifFieldInst) -> Self {
        PartialFieldInfo {
            name: inst.name.to_owned(),
            lsb: inst.partial.0.unwrap_or(0),
            width: inst.width as u16,
            reset: inst.reset.to_u128(inst.width),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PartialFieldInfos(Vec<(String,Vec<PartialFieldInfo>)>);
impl PartialFieldInfos {

    /// Merge all partial info of a field to get the whole width and reset value
    pub fn merge(&self, field_name: &str, is_signed: bool) -> (u16,Vec<ResetVal>) {
        let mut width = 0_u16;
        let mut resets : Vec<ResetVal> = Vec::with_capacity(self.0.len());
        for group in self.0.iter() {
            let mut reset = 0_u128;
            // TODO: check fully defined
            for f in group.1.iter().filter(|f| f.name == field_name) {
                let width_l = f.lsb + f.width;
                if width_l > width {
                    width = width_l;
                }
                reset |= f.reset << f.lsb;
            }
            resets.push(
                if is_signed {ResetVal::Signed(reset as i128)}
                else {ResetVal::Unsigned(reset)}
            );
        }
        (width,resets)
    }

}

#[derive(Clone, Debug)]
pub struct PartialFieldDict(BTreeMap<String, PartialFieldInfos>);

impl PartialFieldDict {

    pub fn new() -> Self {
        PartialFieldDict(BTreeMap::new())
    }

    /// Push Field info into dictionnary
    pub fn push(&mut self, group_type: &str, group_name: &str, info: PartialFieldInfo) -> usize {
        if let Some(infos) = self.0.get_mut(group_type) {
            if let Some(group) = infos.0.iter_mut().find(|e| e.0==group_name) {
                group.1.push(info);
            } else {
                infos.0.push((group_name.to_owned(), vec![info]));
            }
            infos.0.len() - 1
        } else {
            self.0.insert(
                group_type.to_owned(),
                PartialFieldInfos(vec![(group_name.to_owned(), vec![info])])
            );
            0
        }
    }

    pub fn get(&self, group_type: &str) -> Option<&PartialFieldInfos> {
        self.0.get(group_type)
    }
}

impl RifmuxInst {
    // pub fn new(inst_name: String, type_name: String , addr_width: u8, interface: Interface, description: Description, groups: Vec<RifmuxGroupInst>) -> Self {
    pub fn new(inst_name: String, rifmux: &Rifmux, groups: Vec<RifmuxGroupInst>) -> Self {
        RifmuxInst {
            inst_name,
            type_name: rifmux.name.clone(),
            addr_width: rifmux.addr_width,
            data_width: rifmux.data_width,
            sw_clocking: rifmux.sw_clocking.clone(),
            interface: rifmux.interface.clone(),
            description: rifmux.description.clone(),
            components: Vec::new(),
            top: rifmux.top.clone(),
            groups
        }
    }

    pub fn build(src: &RifGenSrc, inst_name: &str, rifmux: &Rifmux, top_params: &ParamValues, suffixes: &HashMap<String,SuffixInfo>) -> Result<Self, String> {
        // println!("RIF Mux = {s} -> \n{def:?}");
        let params = ParamValues::from_iter(rifmux.parameters.items())?;
        let groups = RifmuxGroupInst::from(&rifmux.groups, &params);
        let mut rm = RifmuxInst::new(inst_name.to_owned(), rifmux, groups);
        let mut inst_addr = InstAddr::new(0);
        for i in &rifmux.items {
            let addr = inst_addr.updt(i.addr.value(&params) /*+ group_offset*/, i.addr_kind);
            let mut i_params = ParamValues::new();
            for (k,v) in top_params.items() {
                let mut ks = k.split('.');
                if ks.next() == Some(&i.name) {
                    let param_name = ks.next().ok_or("Malformed parameter overload".to_owned())?;
                    i_params.insert(param_name.to_owned(), *v);
                }
            }
            i_params.compile(i.parameters.iter())?;
            match &i.rif_type {
                RifType::Rif(typename) => {
                    if let Some(rif_def) = src.get_rif(typename) {
                        let rif_suffix = suffixes.get(&i.name).or_else(||i.suffixes.get("")).cloned();
                        // if rif_suffix.is_some() {println!("Found Suffix {:?} for {}", rif_suffix, i.name);}
                        // if !i_params.is_empty() {println!("Parameter in sub-rif {}.{} = {}", rifmux.name, i.name, i_params);}
                        let inst = RifInst::new(&i.name, rif_def, &i_params, &src.rifs, i.description.clone(), rif_suffix)?;
                        rm.components.push(CompInst::new_rif(inst, addr, i.group.clone()));
                    } else if let Some(rifmux) = src.get_rifmux(typename) {
                        // if !i_params.is_empty() {println!("Parameter in sub-rifmux {}.{} = {:?}", rifmux.name, i.name, i_params);}
                        let inst = RifmuxInst::build(src, &i.name, rifmux, &i_params, &i.suffixes)?;
                        rm.components.push(CompInst::new_mux(inst, addr, i.group.clone()));
                    } else {
                        return Err(format!("No RIF definition found for {typename} in {inst_name} ! Available RIFs are: {:?}", src.rifs.keys().collect::<Vec<&String>>()));
                    }
                }
                RifType::Ext(w) => {
                    let ext = RifExt { inst_name: i.name.clone(), addr_width: *w, description: i.description.clone() };
                    rm.components.push(CompInst::new_ext(ext, addr, i.group.clone()));
                }
            }
            rm.components.sort_unstable_by_key(|k| k.full_addr(&rm.groups));
        }
        Ok(rm)
    }
}

impl Comp {

    /// Compile the RIF definition into instances
    pub fn compile(src: &RifGenSrc, suffixes: &HashMap<String, SuffixInfo>, params: &ParamValues) -> Result<Self, String> {
        let r: Comp;
        match &src.top {
            RifGenTop::Rifmux(s) => {
                let Some(def) = src.get_rifmux(s) else {return Err(format!("Rifmux {s} not defined !"));} ;
                r = Comp::Rifmux(RifmuxInst::build(src, s, def, params, suffixes)?);
            }
            RifGenTop::Rif(s) => {
                if let Some(rifdef) = src.rifs.get(s) {
                    r = Comp::Rif(RifInst::new(s, rifdef, params, &src.rifs, rifdef.description.clone(), None)?);
                } else {
                    return Err(format!("Rif {s} not defined !"));
                }
            }
            RifGenTop::None => {
                return Err("No top defined !".to_owned());
            }
        }
        Ok(r)
    }

    pub fn get_name(&self) -> &str {
        match self {
            Comp::Rifmux(r)   => &r.inst_name,
            Comp::Rif(r)      => &r.inst_name,
            Comp::External(r) => &r.inst_name,
        }
    }

    pub fn get_type(&self) -> &str {
        match self {
            Comp::Rifmux(r)   => &r.type_name,
            Comp::Rif(r)      => &r.type_name,
            Comp::External(_) => "",
        }
    }

    pub fn get_addr_width(&self) -> u8 {
        match self {
            Comp::Rifmux(r)   => r.addr_width,
            Comp::Rif(r)      => r.addr_width,
            Comp::External(r) => r.addr_width,
        }
    }

    pub fn get_desc_short(&self) -> &str {
        match self {
            Comp::Rifmux(r)   => r.description.get_short(),
            Comp::Rif(r)      => r.description.get_short(),
            Comp::External(r) => r.description.get_short(),
        }
    }

}
