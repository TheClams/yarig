use std::{format, fs::create_dir_all, path::PathBuf};

use crate::{comp::comp_inst::{Comp, RifFieldInst, RifInst, RifRegInst, RifmuxInst}, parser::remove_rif, rifgen::Access};

use super::{casing::{Casing, ToCasing}, gen_common::{GeneratorBaseSetting, RifList}};


pub struct GeneratorC {
    base_settings: GeneratorBaseSetting,
    base_addr_name: String,
    txt: String,
    stash: String,
}

impl GeneratorC {

    pub fn new(args: GeneratorBaseSetting, base_addr_name: String) -> Self {
        GeneratorC {
            base_settings: args,
            base_addr_name,
            txt: String::with_capacity(10000),
            stash: String::with_capacity(1000)
        }
    }

    fn write(&mut self, string: &str) {
        self.txt.push_str(string);
    }

    fn push_stash(&mut self, string: &str) {
        self.stash.push_str(string);
    }

    fn pop_stash(&mut self) {
        self.txt.push_str(&self.stash);
        self.stash.clear();
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

    pub fn gen(&mut self, obj: &Comp ) -> Result<(), Box<dyn std::error::Error>> {
        // Create output directory if it does not exist
        create_dir_all(self.base_settings.path.clone())?;
        // Call relevant generator (Rif or Rifmux)
        match obj {
            Comp::Rif(rif) => self.gen_rif_c_header(rif)?,
            Comp::Rifmux(rifmux) => {
                let rif_list = RifList::new(rifmux);
                self.gen_rifmux_c_header(rifmux, &rif_list)?;
                if !self.base_settings.gen_inc.is_empty() {
                    for rif in rif_list.iter() {
                        if !self.base_settings.gen_inc.contains(&rif.inst_name) && self.base_settings.gen_inc.first()!=Some(&"*".to_owned()) {
                            continue;
                        }
                        self.gen_rif_c_header(rif)?;
                    }
                }
            },
            // Nothing to do for external RIF
            Comp::External(_) => {},
        }
        Ok(())
    }


    /// C header definition for each RIF
    ///  - one struct by register grouping together fields
    ///  - one struct per page grouping registers
    fn gen_rif_c_header(&mut self, rif: &RifInst) -> Result<(), Box<dyn std::error::Error>> {
        let rifname = &rif.type_name.to_lowercase();
        let rifname_uc = rifname.to_uppercase();
        let basename = remove_rif(rifname);
        // Write header
        self.write(&format!("// Register definition for P_{rifname_uc}\n"));
        self.write(&format!("#ifndef __{rifname_uc}_H__\n"));
        self.write(&format!("#define __{rifname_uc}_H__\n\n"));

        let w = rif.data_width;
        let nb_byte = (w>>3) as u64;
        let type_reg = format!("uint{w}_t");

        // Add enum declaration
        for def in rif.enum_defs.iter() {
            if def.name.starts_with("doc:") {
                continue;
            }
            let mut etn = match def.name.rfind("::") {
                Some(pos) => &def.name[pos+2..],
                None => &def.name,
            };
            if etn.starts_with("e_") {
                etn = &etn[2..];
            }
            let etn = format!("{basename}_{etn}_t");
            self.write(&format!("/// {}\n", def.description));
            self.write(&format!("typedef enum {etn} {{\n"));
            for (i,entry) in def.iter().enumerate() {
                let sep = if i==def.len()-1 {""} else {","};
                self.write(&format!("    {}_{} = {}{} //!< {}\n",
                    basename.to_uppercase(),
                    entry.name.to_uppercase(),
                    entry.value,
                    sep,
                    entry.description.get_short()
                ));
            }
            self.write(&format!("}} {etn};\n\n"));
        }

        for page in rif.pages.iter() {
            if page.is_external() {
                continue;
            }
            let pname =
                if rif.pages.len() > 1 {format!("{}_{}", basename,page.name)}
                else {basename.to_owned()};
            let pname    = pname.to_lowercase();
            let pname_uc = pname.to_uppercase();

            // Add register definition
            for reg in page.iter_reg_type() {
                // Check if register is hidden/reserved in all instances
                if reg.sw_access == Access::NA {
                    continue;
                }

                let reg_type = reg.reg_type.to_lowercase();
                let max_len = reg.fields.iter().map(|f| f.name.len()).max().expect("Registers should have fields");

                self.write(&format!("/// {} {} register bitfields\n", pname.to_casing(Casing::Title), reg.reg_type.to_casing(Casing::Title)));
                for l in reg.base_description.get().lines() {
                    self.write(&format!("/// {l}\n"));
                }
                self.write(&format!("typedef union {pname}_{reg_type}_reg {{\n"));
                self.write(&format!("  {type_reg} reg{w}; //!< Direct access to the full {reg_type} register\n"));
                self.write("  struct {\n");
                let mut pos_l = 0;
                for f in reg.fields.iter() {
                    // Check if field is hidden/reserved in all instances
                    // Fill unused part of the register
                    if pos_l != f.lsb {
                        self.add_field_decl(w, max_len, &format!("rsvd{pos_l}"),f.lsb - pos_l, "Reserved", None);
                    }
                    pos_l = f.lsb + f.width;
                    // Change name if field is marked as reserved and hidden is enabled
                    let name = self.get_field_name(reg, f).to_casing(self.base_settings.casing);
                    let mask = Some((((1_u128<<f.width)-1)<<f.lsb) as usize);
                    let desc = f.base_description.get_short(); // TODO: handle visibility/privacy
                    self.add_field_decl(w, max_len, &name, f.width, desc, mask);
                }
                // Fill remaining bits if any
                if pos_l < w {
                    self.add_field_decl(w, max_len, &format!("rsvd{pos_l}"),w - pos_l, "Reserved", None);
                }
                self.write("  } fields; //!< Access to bitfields\n");
                self.write(&format!("}} {pname}_{reg_type}_reg_t;\n\n"));

                // Optional macro for each fields
                // if args.macro_field {}
                self.write("\n#ifndef DOXYGEN_SHOULD_SKIP_THIS\n");
                for f in reg.fields.iter() {
                    let fieldname = self.get_field_name(reg, f).replace('_', "").to_uppercase();
                    let regname = reg.reg_type.to_uppercase();
                    let name = format!("{pname_uc}_{regname}_{fieldname}", );
                    self.write(&format!("#define {name}_POS   {}\n",f.lsb));
                    self.write(&format!("#define {name}_MASK  0x{:08X}\n",(1_u128<<f.width)-1));
                    self.write(&format!("#define {name}_SMASK ({name}_MASK<<{name}_POS)\n"));
                }
                self.write("#endif /* DOXYGEN_SHOULD_SKIP_THIS */\n\n");
            }

            //  Add one structure for the whole page
            let len_name = page.regs.iter().map(|r| r.reg_name.len()).max().expect("Page should have registers");
            let len_type = 6+page.regs.iter().map(|r| r.reg_type.len()).max().expect("Page should have registers");
            self.push_stash(&format!("/// {} module struct\n", Casing::Title.format(&pname)));
            for l in page.description.get().lines() {
                self.push_stash(&format!("/// {l}\n"));
            }
            self.push_stash(&format!("typedef struct {pname}_regs {{\n"));
            let mut is_union = false;
            let mut addr = 0;
            let mut regs = page.regs.iter().peekable();
            while let Some(reg) = regs.next() {
                // Ignore array element other than the first
                if reg.array.idx() > 0 {
                    continue;
                }
                let reg_type = reg.reg_type.to_lowercase();
                // Detect end of union
                if is_union && reg.addr >= addr {
                    self.push_stash("   };\n");
                    is_union = false;
                }
                // Detect overlaping register
                if !is_union {
                    if let Some(reg_next) = regs.peek() {
                        if reg.addr == reg_next.addr && !is_union {
                            self.push_stash("   union {\n");
                            is_union = true;
                        }
                    }
                }
                // println!("Register {}.addr={} vs {} -> is_union={}", reg.name(), reg.addr, addr, is_union);
                // Detect non-contiguous register: TODO add field overlap to register
                if reg.addr > addr {
                    let span = (reg.addr - addr) / nb_byte;
                    let name = if span > 1 {
                        format!("rsvd{addr}[{span}]")
                    } else {
                        format!("rsvd{addr}")
                    };
                    self.push_stash(&format!("  {type_reg}{spc} {name:<len_name$};\n",
                        spc = " ".repeat(len_type+pname.len() + 1 - type_reg.len())));
                }
                // Add register instance
                if is_union {
                    self.push_stash("  ");
                }
                let desc = if reg.array.dim() > 1 {
                    reg.base_description.get_short()
                } else {
                    reg.description.get_short()
                };
                let mut reg_name = reg.reg_name.to_lowercase();
                if reg.array.dim() > 1 {
                    reg_name.push_str(&format!("[{}]",reg.array.dim()));
                };
                self.push_stash(
                    &format!("  {pname}_{rtype:<len_type$} {reg_name:<len_name$}; //!< 0x{addr:04X} (0x{rst:08X} {access}): {desc}\n",
                        rtype = &format!("{reg_type}_reg_t"),
                        addr = reg.addr,
                        rst = reg.reset,
                        access = reg.sw_access,
                ));
                // Calculate expected next address
                let nb = reg.array.dim().max(1);
                addr = reg.addr + nb_byte * nb as u64;
            }
            if is_union {
                self.push_stash("   };\n");
            }
            self.push_stash(&format!("}} {pname}_regs_t;\n\n"));

            // Optional macro for each register instance
            //
            // if args.macro_field {}
            self.push_stash("\n#ifndef DOXYGEN_SHOULD_SKIP_THIS\n");
            for reg in page.regs.iter() {
                // Ignore array element other than the first
                if reg.array.idx() > 0 {
                    continue;
                }
                let reg_name = reg.reg_name.to_uppercase();
                self.push_stash(
                    &format!("#define {pname_uc}_{reg_name}_OFFSET {addr}\n",
                        addr = page.addr + reg.addr)
                );
                self.push_stash(&format!("#define {pname_uc}_{reg_name}_RESET {rst:#08X}\n", rst = reg.reset));
            }
            self.push_stash("#endif /* DOXYGEN_SHOULD_SKIP_THIS */\n\n");
        }

        self.pop_stash();
        self.write(&format!("#endif /* __{rifname_uc}_H__ */\n"));

        // Write file
        self.save(&format!("{}.h",rif.name(false).to_lowercase()))
    }

    fn get_field_name(&self, r: &RifRegInst, f: &RifFieldInst) -> String {
        if f.is_reserved() && self.base_settings.privacy.is_public() {
            format!("rsvd{}",f.lsb)
        } else if f.array.dim() > 1 || r.array.dim()==0 || r.array.is_inst() {
            f.name_flat()
        } else {
            f.name.to_owned()
        }
    }

    fn add_field_decl(&mut self, reg_width: u8, l:usize, name: &str, field_width: u8, desc: &str, mask: Option<usize>) {
        let mask = if let Some(v) = mask {format!("0x{v:08X} ")} else {"".to_owned()};
        self.write(&format!("    uint{reg_width}_t {name:<l$} : {field_width:>2}; //!< {mask}{desc}\n"));
    }

    fn gen_rifmux_c_header(&mut self, rifmux: &RifmuxInst, rif_list: &RifList) -> Result<(), Box<dyn std::error::Error>> {
        let rifname = &rifmux.inst_name;
        let rifname_uc = rifname.to_uppercase();
        self.txt.clear();
        // Write header
        self.write("// Register File mapping\n");
        self.write(&format!("#ifndef __{rifname_uc}_H__\n"));
        self.write(&format!("#define __{rifname_uc}_H__\n\n"));

        // Includes
        self.write("// Includes Register File definition\n");
        for rif in rif_list.iter() {
            self.write(&format!("#include \"{}.h\"\n", rif.name(false).to_lowercase()));
        }
        self.write("\n");

        // Mapping
        // for group in rifmux.groups.iter() {
        //     self.write(&format!("#define {prefix}_{name} ({prefix} + 0x{addr:08X})\n", prefix=self.base_addr_name, name=group.name, addr=group.addr));
        // }
        self.add_ptr_rifmux(rifmux, "", 0);
        self.pop_stash();
        self.write("\n");

        self.write(&format!("#endif /* __{rifname_uc}_H__ */\n"));
        // Write file
        self.save(&format!("{rifname}.h"))
    }

    fn add_ptr_rifmux(&mut self, rifmux: &RifmuxInst, top_name: &str, offset: u64) {
        let prefix = if top_name.is_empty() {
            "".to_owned()
        } else {
            format!("{}_",top_name)
        };
        // println!("Groups of {} = {:#?}", rifmux.inst_name, rifmux.groups);
        for comp in rifmux.components.iter() {
            let mut base_addr_name = self.base_addr_name.clone();
            if !comp.group.is_empty() && prefix.is_empty() {
                base_addr_name.push('_');
                base_addr_name.push_str(&comp.group);
            }
            match &comp.inst {
                Comp::Rifmux(r) => {
                    let comp_name = format!("{prefix}{}",r.inst_name.to_casing(Casing::Pascal));
                    self.add_ptr_rifmux(r, &comp_name, offset + comp.addr)
                }
                Comp::Rif(r) => {
                    let rif_inst_name = remove_rif(&r.inst_name).replace('_', "");
                    for page in r.pages.iter() {
                        let mut page_name = format!("{prefix}{rif_inst_name}");
                        let mut name_tt = format!("{prefix}{}",remove_rif(&r.inst_name)).to_casing(Casing::Title);
                        let mut page_type = remove_rif(&r.type_name).to_owned();
                        if r.pages.len() > 1 {
                            page_name.push_str(&page.name.replace('_', ""));
                            name_tt.push(' ');
                            name_tt.push_str(&page.name.to_casing(Casing::Title));
                            page_type.push_str(&format!("_{}",page.name));
                        }
                        let name_uc = page_name.to_uppercase();
                        let page_type = page_type.to_lowercase();
                        let desc = if page.description.is_empty() {r.description.get_short()} else {page.description.get_short()};
                        let addr = page.addr + comp.addr + offset;
                        // let addr = page.addr + if comp.group.is_empty() {comp.addr} else {0};
                        self.write(&format!("/// {name_tt} base address: {desc}\n"));
                        self.write(&format!("#define {name_uc}_BASE_ADDR ({base_addr_name} + 0x{addr:08X})\n"));
                        self.push_stash(&format!("/// Pointer to {name_tt} registers\n"));
                        self.push_stash(&format!("#define P_{name_uc} ((volatile {page_type}_regs_t* ) {name_uc}_BASE_ADDR)\n"));
                    }
                    self.write("\n");
                }
                Comp::External(_) => {},
            }
        }
    }
}
