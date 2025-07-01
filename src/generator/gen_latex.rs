use crate::{generator::casing::ToCasing, rifgen::EnumDef};

use super::{casing::Casing, gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore}, trait_doc::{CellKind, GeneratorDoc, TableKind}};


use yarig_macro::add_gen_core;

#[add_gen_core("tex")]
pub struct GeneratorLatex {
    /// Flag when next table column is the first of a row
    first_col: bool,
}

impl GeneratorLatex {

    pub fn new(setting: GeneratorBaseSetting) -> Self {
        GeneratorLatex {
            core: GeneratorCore::new(0,setting),
            first_col: true,
        }
    }

}

impl GeneratorDoc for GeneratorLatex {
    const HAS_LAYOUT : bool = false;
    const SHOW_TYPE  : bool = false;
    const SHOW_RESET : bool = false;
    const SHOW_SINGLE_REG : bool = false;
    const SHOW_UNUSED : bool = true;


    fn write_rif_title(&mut self, idx_rif: (&str,usize), desc: &str) {
        // Show title only if sub-paragraph (i.e. RIFmux element)
        // println!("Title RIF {} : {} | rifmux={}", idx_rif.1, idx_rif.0, self.comp().is_rifmux);
        if idx_rif.1 > 0 && self.comp().is_rifmux {
            self.write("\\subsection{");
            if desc.is_empty() {
                self.write(&self.sanitize(idx_rif.0));
            } else {
                self.write(desc);
            }
            self.write("}");
        } else if !desc.is_empty() {
            self.write(desc);
        }
        self.write("\n");
        if !self.comp().is_rifmux {
            self.write("\t\\subsection{Address mapping}\n");
        }
    }

    fn write_page_title(&mut self, _idx_rif: (&str,usize), idx_page: (&str, usize), desc: (String, Option<String>)) {
        if self.comp().cnt > 1 {
            let k = if self.comp().is_rifmux {"*"} else {""};
            self.write(&format!("\t\\subsection{k}{{"));
            let name = self.sanitize(idx_page.0);
            if desc.0.is_empty() {
                self.write(&name);
            } else {
                self.write(&format!("{} ({name})", desc.0));
            }
            self.write("}\n");
        } else if !desc.0.is_empty() {
            self.write(&self.sanitize(&desc.0));
            self.write("\n");
        }
        if let Some(desc_detail) = desc.1 {
            self.write_info(&self.sanitize(&desc_detail));
            self.write("\n");
        }
    }

    fn write_reg_title(&mut self, _idx_rif: (&str,usize), _idx_page: (&str, usize), idx_reg: (&str, usize), desc: &str, base_addr: Option<u64>) {
        self.write("\t\\subsubsection{");
        let name = self.sanitize(idx_reg.0);
        if desc.is_empty() {
            self.write(&name);
        } else {
            self.write(&format!("{desc} ({name})"));
        }
        if let Some(addr) = base_addr {
            let w = ((self.addr_width()+3) >> 2) as usize;
            self.write(&format!(" @ 0x{addr:0w$x}"));
        }
        self.write("}\n");
    }

    fn write_table_title(&mut self, kind: TableKind, title: &str, id: &str) {
        let title = self.sanitize(title);
        let caption = match kind {
            TableKind::Page => {
                if self.comp().cnt > 1 {
                    format!("{} registers address mapping", title.to_casing(Casing::Title))
                } else {
                    "Registers address mapping".to_owned()
                }
            }
            TableKind::Field => format!("Register {title}"),
            _ => title
        };
        let id_short = id.strip_prefix("fields.").unwrap_or(id).replace(['[',']'], "");
        self.write(&format!("\t\\begin{{longtblr}}[caption={{{caption}}},label={{rif:{id_short}}}]"));
        self.write("{rowhead=1,row{1}={bg=colorTableHeading,c,font=\\bfseries},hlines,vlines" );
        match kind {
            TableKind::Rifmux  => self.write(",colspec={p{1.80cm}p{4.30cm}p{9.0cm}}}"),
            TableKind::RifInst => self.write(",colspec={p{1.80cm}p{4.30cm}p{9.0cm}}}"),
            TableKind::Page    => self.write(",colspec={p{1.80cm}p{4.30cm}p{9.00cm}}}"),
            TableKind::RegInst => self.write(",colspec={p{1.80cm}p{2.90cm}p{1.80cm}p{8.50cm}}}"),
            TableKind::Field   => self.write(",colspec={p{0.90cm}p{2.90cm}p{1.30cm}p{1.80cm}p{8.00cm}}}"),
            _  => self.write("}"),
        }
        self.write("\n");
    }

    fn write_table_footer(&mut self, kind: TableKind) {
        self.write("\t\\end{longtblr}\n");
        if kind==TableKind::Page && self.comp().cnt <= 1 && !self.comp().is_rifmux {
            self.write("\t\\subsection{Registers definition}\n");
        }
    }

    fn write_table_row_header(&mut self, id: Option<&str>) {
        self.first_col = true;
        self.write("\t\t");
        if let Some(id) = id {
            let id_ = id.replace(['[',']'], "");
            self.write(&format!("\\refstepcounter{{rif}}\\label{{rif:{id_}}} "));
        }
    }

    fn write_table_row_footer(&mut self) {
        self.write("\\\\\n")
    }

    fn write_table_cell(&mut self, kind: (TableKind, CellKind), span: usize, txt: &str, _id: &str, _tip: Option<String>) {
        if !self.first_col {self.write(" & ");}
        self.first_col = false;
        // Call sanitize on non-description cell (already caled on description)
        let txt = if kind.1!=CellKind::Desc {self.sanitize(txt)} else {txt.to_owned()};
        if span > 1 {
            self.write(&format!("\\SetCell[c={span}]{{{txt}}}"));
        } else if kind.0==TableKind::FieldRsvd && txt.is_empty() {
            match kind.1 {
                CellKind::Inst => self.write("\\SetCell[c=1]{c}{-}"),
                CellKind::Desc => self.write(" \\textit{Reserved}"),
                _s => {}
            }
        } else if txt.contains("\\\\") {
            self.write(&format!("{{{txt}}}"));
        } else {
            self.write(&txt);
        }
    }

    fn enum_def_desc(&mut self, def: &EnumDef) -> String {
        let mut desc = String::with_capacity(def.len() * 16);
        desc.push_str("{\\small ");
        let mut enum_iter = def.values.iter().peekable();
        while let Some(entry) = enum_iter.next() {
            let entry_name = self.sanitize(&entry.name);
            let entry_desc = self.sanitize(&entry.description.get(self.is_public()));
            desc.push_str(&format!("{:3} - {entry_name} : {entry_desc}", entry.value));
            if enum_iter.peek().is_some() {
                desc.push_str("\\\\");
            }
        }
        desc.push('}');
        desc
    }

    fn sanitize(&self, raw: &str) -> String {
        let mut txt = String::with_capacity(raw.len());
        // Latex equation are enclosed between backtick:
        // Only handle special character outside of equations
        let mut is_eq = false;
        for raw_part in raw.split('`') {
            // println!("Desc part = {raw_part} ({is_eq})");
            if is_eq {
                txt.push('$');
                txt.push_str(raw_part);
                txt.push('$');
            } else {
                let mut chars = raw_part.chars().peekable();
                while let Some(c) = chars.next() {
                    let cn = chars.peek();
                    match c {
                        // Latex equation is enclosed between backtick
                        '`' => {
                            while let Some(c) = chars.next_if(|c| c!=&'`') {
                                txt.push(c);
                            }
                        }
                        // Characters to escape
                        '_' | '&' | '%' | '#' | '{' | '}' => {
                            txt.push('\\');
                            txt.push(c);
                        }
                        // SuperScript sequence
                        '^'  => {
                            if let Some(cn) = cn {
                                if cn.is_alphanumeric() || cn==&'-' || cn==&'+' {
                                    txt.push_str("\\textsuperscript{");
                                    txt.push(*cn);
                                    chars.next();
                                    while let Some(c) = chars.next_if(|c| c.is_alphanumeric()) {
                                        txt.push(c);
                                    }
                                    txt.push('}');
                                } else {
                                    txt.push_str("\\^{}");
                                }
                            } else {
                                txt.push_str("\\^{}");
                            }
                        }
                        // Line return
                        '\n' => txt.push_str("\\\\"),
                        // Special character/sequences
                        '~' => txt.push_str("$\\sim$"),
                        '<' if cn==Some(&'<') => {
                            txt.push_str("$<<$");
                            chars.next();
                        }
                        '>' if cn==Some(&'>') => {
                            txt.push_str("$>>$");
                            chars.next();
                        }
                        // Others => copy the character
                        _ => txt.push(c),
                    }
                }
            }
            is_eq = !is_eq;
        }
        txt
    }

    fn write_info(&mut self, info: &str) {
        self.write(info);
        self.write("\n");
    }

}