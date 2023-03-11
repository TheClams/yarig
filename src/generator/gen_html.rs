use std::{collections::HashMap, format, fs::create_dir_all, path::PathBuf};

use crate::{
    comp::comp_inst::{Comp, CompInst, RifFieldInst, RifInst, RifRegInst, RifmuxGroupInst},
    parser::remove_rif, rifgen::FieldSwKind,
};

use super::{casing::ToCasing, gen_common::{GeneratorBaseSetting, Privacy, RifList}};
use super::casing::Casing;

const DEFAULT_CSS : &str = include_str!("resources/style.css");

type InstDict = HashMap<String,Vec<u16>>;

pub struct GeneratorHtml {
    base_settings: GeneratorBaseSetting,
    txt: String,
}

impl GeneratorHtml {

    pub fn new(args: GeneratorBaseSetting) -> Self {
        GeneratorHtml {
            base_settings: args,
            txt: String::with_capacity(10000)
        }
    }

    fn write(&mut self, string: &str) {
        self.txt.push_str(string);
    }

    fn save(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path : PathBuf = [
            self.base_settings.path.clone(),
            filename.into()
        ].iter().collect();
        std::fs::write(path, self.txt.as_bytes())?;
        Ok(())
    }

    //-----------------------------

    pub fn gen(&mut self, obj: &Comp) -> Result<(), Box<dyn std::error::Error>> {
        // Create output directory if it does not exist
        create_dir_all(self.base_settings.path.clone())?;
        // Write header
        self.write(&format!("<!DOCTYPE html>\n<html><head><title>{} RIF Documentation</title>\n",obj.get_name()));
        // Basic script for popup
        self.write("<script type=\"text/javascript\">\n");
        self.write("\tfunction ShowPopup(evt, popupid) {\n");
        self.write("\t\thp = document.getElementById(popupid);\n");
        self.write("\t\thp.style.top = 10 + evt.clientY + (document.documentElement.scrollTop ? document.documentElement.scrollTop : document.body.scrollTop);\n");
        self.write("\t\thp.style.left = 10 + evt.clientX + (document.documentElement.scrollLeft ? document.documentElement.scrollLeft : document.body.scrollLeft);\n");
        self.write("\t\thp.style.visibility = \"Visible\";\n");
        self.write("\t}\n");
        self.write("\tfunction HidePopup(popupid) {\n");
        self.write("\t\thp = document.getElementById(popupid);\n");
        self.write("\t\thp.style.visibility = \"Hidden\"; \n");
        self.write("\t}\n");
        self.write("</script>\n");
        // CSS
        self.write("<style type=\"text/css\">\n");
        self.write(DEFAULT_CSS);
        self.write("</style>\n");
        //
        self.write("</head><body><div class=\"fulldoc\" id=\"top\">\n");

        let top_name;

        match obj {
            Comp::Rifmux(r) => {
                top_name = &r.inst_name;
                // RifMux: Table describing mapping of all its element Other rifmux or RIF
                // Sub-rifmux or expanded to only display RIFs
                self.write(&format!("<h1>{}</h1>\n", Casing::Title.format(&r.inst_name)));
                self.write("<table id=\"rifSummary\"><caption>Summary</caption>\n");
                self.write("<tr><th>Offset</th><th>Type Name</th><th>Instance Name</th><th>Description</th></tr>\n");
                let rif_list = RifList::new(r);
                let w = ((r.addr_width+3) >> 2) as usize;
                for c in r.components.iter() {
                    self.add_rifmux_entry(c, w,  0, None, &r.groups);
                }
                self.write("</table>\n");
                // Add RIF definition for all RIF type used
                let mut idx = 1;
                for rif in rif_list.iter() {
                    self.add_rif(rif, idx, true)?;
                    idx += 1;
                }
            },
            Comp::Rif(r) => {
                top_name = &r.type_name; // TODO: apply suffix
                self.add_rif(r, 1, false)?;
            },
            // Nothing todo for external RIF
            Comp::External(_) => return Ok(()),
        }

        self.write("</div></body></html>\n");

        // Write file
        self.save(&format!("{top_name}.html"))
    }

    // Add row in table, composed of 4 column:
    // Address, Type name, Instance name and short description
    fn add_rifmux_entry (&mut self, comp: &CompInst, w: usize, offset: u64, top_name: Option<&str>, groups: &[RifmuxGroupInst]) {
        //
        let rif_name = remove_rif(comp.inst.get_name()).to_casing(self.base_settings.casing);
        //
        let instname = if let Some(top) = top_name {format!("{}.{}", top, rif_name)} else {rif_name};
        let addr = comp.full_addr(groups) + offset;
        match &comp.inst {
            Comp::Rifmux(c) => {
                for sub in c.components.iter() {
                    self.add_rifmux_entry(sub, w, addr, Some(&instname), &c.groups);
                }
            },
            Comp::Rif(c) => {
                let tn = remove_rif(&c.type_name);
                self.write(&format!("<tr><td>0x{addr:0w$X}</td>"));
                self.write(&format!("<td><a href=\"#compName__{tn}\">{tn}</a></td>"));
                self.write(&format!("<td>{}</td>",instname));
                self.write(&format!("<td>{}</td></tr>\n",c.description.get_short()));
            }
            Comp::External(c) => {
                self.write(&format!("<tr><td>0x{:0X}</td>",addr));
                self.write("<td>Memory</a></td>");
                self.write(&format!("<td>{}</td>",instname));
                self.write(&format!("<td>{}</td></tr>\n",c.description.get_short()));
            }
        }
    }

    //
    fn add_rif(&mut self, rif: &RifInst, idx: usize, has_top: bool) -> Result<(),String> {
        let rif_name = remove_rif(&rif.type_name);
        self.write(&format!("<h1 id=\"compName__{}\">{}. ",rif_name,idx));
        let desc = rif.base_description.get_split();
        if desc.0.is_empty() {
            self.write(&Casing::Title.format(rif_name));
        } else {
            self.write(desc.0);
        }
        self.write("</h1>\n");
        if let Some(desc_detail) = desc.1 {
            self.write(&format!("<span><p>{}</p></span>\n", self.sanitize(desc_detail)));
        }
        let inst_dict = self.add_reg_summary(rif);
        if has_top {
            self.write("&nbsp;&nbsp;&nbsp;&nbsp;<a href=\"#rifSummary\">return to top</a>\n");
        }
        self.add_reg_detail(rif, &inst_dict, idx)
    }

    //
    fn add_reg_summary(&mut self, rif: &RifInst) -> InstDict {
        let rif_name = remove_rif(&rif.type_name);
        let addr_w = ((rif.addr_width+3)>>2) as usize;
        let data_w = ((rif.data_width+3)>>2) as usize;
        let mut dict : InstDict = HashMap::new();
        for page in rif.pages.iter() {
            // TODO: check hidden
            self.write(&format!("<table id=\"regSummary__{rif_name}_{page_name}\"><caption>{page_name} Summary</caption>\n",page_name=page.name));
            self.write("<tr><th>Offset</th><th>Instance Name</th><th>Reset</th><th>Description</th><th>Register Type</th></tr>\n");
            for (idx,reg) in page.regs.iter().enumerate() {
                if self.base_settings.privacy.is_internal() && reg.visibility.is_hidden() {
                    continue;
                }
                let arr_idx = if reg.array.dim() > 0 {format!("[{}]",reg.array.idx())} else {"".to_owned()};
                let reg_type = reg.expanded_type_name().to_casing(self.base_settings.casing);
                self.write(&format!("<tr><td>0x{addr:0addr_w$X}</td>", addr=page.addr+reg.addr));
                self.write(&format!("<td>{}{arr_idx}</td>", reg.reg_name.to_casing(self.base_settings.casing)));
                self.write(&format!("<td width=\"100\">0x{:0data_w$X}</td>\n", reg.reset));
                self.write(&format!("<td>{}</td>\n", reg.description.get_short()));
                self.write(&format!("<td><a href=\"#regName__{rif_name}_{reg_type}\">{reg_type}</a></td></tr>\n"));
                dict.entry(reg_type).or_default().push(idx as u16);
            }
            self.write("</table>\n");
        }
        dict
    }

    fn add_reg_detail(&mut self, rif: &RifInst, inst_dict: &InstDict, idx_c: usize)  -> Result<(),String> {
        let rifname = remove_rif(&rif.type_name);
        let mut idx_p = 0;
        let nb_col = (32 / rif.data_width).max(1);
        let addr_w = ((rif.addr_width+3)>>2) as usize;
        let data_w = ((rif.data_width+3)>>2) as usize;
        let is_public = self.base_settings.privacy.is_public();
        for page in rif.pages.iter() {
            idx_p += 1;
            // TODO: check hidden
            // Add paragraph per page only if more than one
            if rif.pages.len() > 1 {
                self.write(&format!("<h2 id=\"pageName__{rifname}_{page_name}\">{idx_c}.{idx_p} {page_name}</h2>\n", page_name=page.name));
            }
            let mut idx_r = 0;
            for (idx_ri,reg) in page.regs.iter().enumerate() {
                // Check hidden
                if self.base_settings.privacy.is_internal() && reg.visibility.is_hidden() {
                    continue;
                }
                // Check not already defined in case of compact display
                let reg_type = reg.expanded_type_name().to_casing(self.base_settings.casing);
                let Some(instances) = inst_dict.get(&reg_type) else {
                    return Err(format!("Unable to find register type {} in instance dict: {:?}", reg_type, inst_dict.keys().collect::<Vec<&String>>()))
                };
                if instances.first() != Some(&(idx_ri as u16)) {
                    continue;
                }
                let reg_impl = rif.get_hw_reg(&reg.group_type);
                // Title
                idx_r += 1;
                self.write(&format!("<h3 id=\"regName__{rifname}_{reg_type}\">{idx_c}.{idx_p}.{idx_r} "));
                if reg.base_description.is_empty() {
                    self.write(&format!("{reg_type}</h3>\n"));
                } else {
                    let desc = reg.base_description.get_split();
                    self.write(&format!("{} ({reg_type})</h3>\n",desc.0));
                    if let Some(desc_detail) = desc.1 {
                        self.write(&format!("<span><p>{}</p></span>\n", self.sanitize(desc_detail)));
                    }
                }
                // Register instance summary : Name, offset, reset, Description
                // Only if multiple instance ?
                self.write("<table class=\"noborders\">\n");
                self.write("<tr><th width=\"33%\" class=\"noborders\">Instance Name</th><th class=\"noborders\">Offset</th><th class=\"noborders\">Reset</th>");
                if instances.len() > 1 {
                    self.write("<th>Description</th>");
                }
                self.write("</tr>\n");
                for inst_idx in instances.iter() {
                    let Some(inst) = page.regs.get(*inst_idx as usize) else {
                        return Err(format!("Instance index {} out of range (max {}) !!!", inst_idx, page.regs.len()));
                    };
                    let arr_idx = if inst.array.dim() > 0 {format!("[{}]",inst.array.idx())} else {"".to_owned()};
                    self.write(&format!("<tr><td width=\"33%\" class=\"noborders\">{}{arr_idx}\n", inst.reg_name.to_casing(self.base_settings.casing)));
                    self.write(&format!("<td class=\"noborders\">0x{addr:0addr_w$X}</td>\n", addr=page.addr+inst.addr));
                    self.write(&format!("<td class=\"noborders\">0x{rst:0data_w$X}</td>\n", rst=inst.reset));
                    if instances.len() > 1 {
                        self.write(&format!("<td class=\"noborders\">{}</td>", inst.description.get_short()));
                    }
                    self.write("</tr>\n");
                }

                self.write("</table>\n");
                // Fields Mapping
                self.write("<table class=\"map\">\n");
                self.write("  <tr><td width=\"*\" class=\"map\"></td>\n");
                self.write(&"<td width=\"22\" class=\"map\"></td>".repeat(32));
                self.write("</tr>\n  <tr><td class=\"map\">Bit</td>\n");
                for i in (0..rif.data_width).rev() {
                    self.write(&format!("  <td colspan=\"{nb_col}\" class=\"map\">{i}</td>\n"));
                }
                self.write("</tr>\n  <tr><td class=\"map\">Field</td>\n");
                let mut last_pos = rif.data_width;
                for f in reg.fields.iter().rev().filter(|f| !(f.visibility.is_hidden() && is_public)) {
                    let fieldname = self.get_field_name(reg, f);
                    // Insert reserved in uncoppied bits
                    if f.msb()+1 < last_pos {
                        let w = last_pos - (f.msb()+1);
                        self.write(&format!("<td colspan=\"{}\" class=\"rsvd\"></td>\n", w*nb_col));
                    }
                    let td_class = if (f.width as usize * 3 * nb_col as usize) < fieldname.len() {"mapv"} else {"map"};
                    self.write(&format!("<td colspan=\"{}\" class=\"{td_class}\">{fieldname}</td>\n",f.width*nb_col));
                    last_pos = f.lsb;
                }
                if last_pos!=0 {
                    self.write(&format!("<td colspan=\"{}\" class=\"rsvd\"></td>\n",last_pos*nb_col));
                }
                self.write("</tr>\n  <tr><td class=\"map\">Reset</td>\n");
                for i in (0..rif.data_width).rev() {
                    self.write(&format!("<td colspan=\"{nb_col}\" class=\"map\">{}</td>\n",(reg.reset >> i)&1));
                }
                self.write("  </tr>\n</table>\n");
                let is_intr_derived = reg.intr_info.0.is_derived();
                if !is_intr_derived {
                    // Fields Details
                    self.write("<table><tr><th>Bits</th><th>Name</th><th>Access</th><th>Reset</th><th>Description</th></tr>\n");
                    for f in reg.fields.iter().rev().filter(|f| !(f.visibility.is_hidden() && is_public)) {
                        // Skip hidden field in public documentation
                        if f.visibility.is_hidden() && self.base_settings.privacy==Privacy::Public {
                            continue;
                        }
                        let fieldname = self.get_field_name(reg, f);
                        // Position
                        self.write(&format!("<tr id=\"fieldName__{rifname}_{reg_type}_{fieldname}\">\n"));
                        if f.width==1 {
                            self.write(&format!("<td>{}</td>",f.lsb));
                        } else {
                            self.write(&format!("<td>{}:{}</td>",f.msb(), f.lsb));
                        }
                        // Name, Access, Reset
                        let access = match f.sw_kind {
                            FieldSwKind::ReadWrite   => "RW",
                            FieldSwKind::ReadOnly    => "RO",
                            FieldSwKind::WriteOnly   => "WO",
                            FieldSwKind::ReadClr     => "RCLR",
                            FieldSwKind::W1Clr       => "W1CLR",
                            FieldSwKind::W0Clr       => "W0CLR",
                            FieldSwKind::W1Set       => "W1SET",
                            FieldSwKind::W1Tgl       => "W1TGL",
                            FieldSwKind::W1Pulse(_,_) => "Pulse",
                            FieldSwKind::Password(_) => "WO",
                        };
                        self.write(&format!("<td>{fieldname}</td><td>{access}</td><td>"));
                        // Check if the field reset is the same in all register instance
                        // If not display a dash character
                        let mut is_single_reset = true;
                        for inst_idx in instances.iter().skip(1) {
                            let reg_inst = page.regs.get(*inst_idx as usize).unwrap(); // Case were this does not exist already checked before
                            let Some(f_inst) = reg_inst.fields.iter().find(|fi| fi.name==f.name) else {
                                return Err(format!("Unable to find field {}.{} !", reg.reg_name, f.name));
                            };
                            if f_inst.reset != f.reset {
                                is_single_reset = false;
                                self.write("-");
                                break;
                            }
                        }
                        if is_single_reset {
                            let reset = f.reset.to_u128(f.width);
                            let w = (f.width>>2) as usize;
                            self.write(&format!("0x{reset:0w$X}"))
                        }
                        self.write("</td>\n<td>");
                        // Description
                        self.write("<span>");
                        self.write(&self.sanitize(f.description.get()));
                        if let Some(enum_name) = f.enum_kind.name() {
                            let name = if let Some(pkg) = &reg_impl.pkg {
                                if enum_name.contains(':') {enum_name.to_owned()}
                                else {format!("{pkg}_pkg::{enum_name}")}
                            } else {
                                enum_name.to_owned()
                            };
                            let enum_def = rif.get_enum_def(&name)?;
                            self.write("<table class=\"noborders\">\n");
                            for e in enum_def.iter() {
                                self.write(&format!("<tr><td width=\"40\" class=\"noborders\">{:#0x}</td>", e.value));
                                self.write(&format!("<td width=\"*\" class=\"noborders\">{}</td></tr>",e.description.get()));
                            }
                            self.write("</table>\n");
                        }
                        self.write("</span>\n");
                    }
                }
                self.write("</tr>\n</table>\n");
                self.write(&format!("&nbsp;&nbsp;&nbsp;&nbsp;<a href=\"#regSummary__{rifname}_{page_name}\">return to summary</a>", page_name=page.name));
                if is_intr_derived {
                    self.write(&format!("&nbsp;&nbsp;&nbsp;&nbsp;<a href=\"#regName__{rifname}_{regtype}\">Fields details</a>", regtype=reg.reg_type));
                }
                self.write("\n");
            }
        }
        Ok(())
    }

    /// Sanitize text for HTML
    /// Replace \n by <br/>
    /// Replace indenting space by &nbsp;
    fn sanitize(&self, desc: &str) -> String {
        let mut txt = String::with_capacity(desc.len());
        for l in desc.split('\n') {
            // Insert a line return after each line
            if !txt.is_empty() {
                txt.push_str("<br/>");
            }
            // Replace starting indentation by &nbsp; to
            let nb_spc = l.chars().take_while(|c| c.is_whitespace()).count();
            txt.push_str(&"&nbsp;".repeat(nb_spc));
            //
            txt.push_str(l.trim());
        }
        txt
    }

    fn get_field_name(&self, r: &RifRegInst, f: &RifFieldInst) -> String {
        // println!("[get_field_name] {}.{} : rsvd={}, field array = {:?}, reg array={:?}",
        //     r.reg_name, f.name, f.is_reserved(), f.array, r.array);
        if f.is_reserved() && self.base_settings.privacy.is_public() {
            format!("rsvd{}",f.lsb)
        } else if f.array.dim() > 1 || r.array.dim()==0 || r.array.is_inst() {
            f.name_flat().to_casing(self.base_settings.casing)
        } else {
            f.name.to_casing(self.base_settings.casing)
        }
    }

}