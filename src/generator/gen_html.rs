use crate::rifgen::EnumDef;

use super::{casing::{Casing, ToCasing}, gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore}, trait_doc::{CellKind, GeneratorDoc, LinkKind, TableKind}};

#[allow(dead_code)]
pub struct GeneratorHtml {
    /// Base structure of all generators
    core: GeneratorCore,
    /// Number of column per bit for each register
    nb_col : usize,
    /// Flag when current RIF has multiple pages
    multipage : bool,
}

#[allow(dead_code)]
impl GeneratorHtml {
    const DEFAULT_CSS : &'static str = include_str!("resources/style.css");

    pub fn new(setting: GeneratorBaseSetting) -> Self {
        GeneratorHtml {
            core: GeneratorCore::new(0,setting),
            nb_col: 1,
            multipage: false
        }
    }
}

impl GeneratorBase for GeneratorHtml {

    const EXT : &'static str = "html";

    fn core(&self) -> &GeneratorCore {
        &self.core
    }

    fn core_mut(&mut self) -> &mut GeneratorCore {
        &mut self.core
    }

}

impl GeneratorDoc for GeneratorHtml {
    const HAS_LAYOUT : bool = true;
    const SHOW_RESET : bool = true;
    const SHOW_TYPE : bool = false;
    const SHOW_SINGLE_REG : bool = true;

    fn set_rif_info(&mut self, _addr_w: u8, data_w: u8, nb_page: usize) {
        self.nb_col = 32 / data_w as usize;
        self.multipage = nb_page > 1;
    }

    fn write_header(&mut self, name: &str) {
        self.write(&format!("<!DOCTYPE html>\n<html><head><title>{} RIF Documentation</title>\n", name.to_casing(Casing::Title)));
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
        self.write(Self::DEFAULT_CSS);
        self.write("</style>\n");
        //
        self.write("</head><body><div class=\"fulldoc\" id=\"top\">\n");    }

    fn write_rif_title(&mut self, idx_rif: (&str,usize), desc: &str) {
        self.write("<h1" );
        if idx_rif.1 != 0 {
            self.write(&format!(" id=\"{}\"", idx_rif.0));
        }
        self.write(">" );
        if idx_rif.1 != 0 {
            self.write(&format!("{}. ", idx_rif.1));
        }
        if desc.is_empty() {
            self.write(&idx_rif.0.to_casing(Casing::Title));
        } else {
            self.write(desc);
        }
        self.write("</h1>\n");
    }

    fn write_page_title(&mut self, idx_rif: (&str,usize), idx_page: (&str, usize), desc: (&str, Option<&str>)) {
        if !self.multipage {
            return;
        }
        self.write(&format!("<h2 id=\"{}.{}\">", idx_rif.0, idx_page.0));
        self.write(&format!("{}.{} ", idx_rif.1, idx_page.1));
        if desc.0.is_empty() {
            self.write(&idx_page.0.to_casing(Casing::Title));
        } else {
            self.write(desc.0);
        }
        self.write("</h2>\n");
        if let Some(desc_detail) = desc.1 {
            self.write_info(desc_detail);
        }
    }

    fn write_reg_title(&mut self, idx_rif: (&str,usize), idx_page: (&str, usize), idx_reg: (&str, usize), desc: &str) {
        self.write(&format!("<h3 id=\"{}.{}\">", idx_rif.0, idx_reg.0));
        self.write(&format!("{}.{}.{} ", idx_rif.1, idx_page.1, idx_reg.1));
        if desc.is_empty() {
            self.write(idx_page.0);
        } else {
            self.write(&format!("{desc} ({})", idx_reg.0));
        }
        self.write("</h3>\n");
    }

    fn write_table_title(&mut self, kind: TableKind, title: &str, id: &str) {
        self.write("<table" );
        match kind {
            TableKind::RegInst => self.write(" class=\"noborders\""),
            TableKind::Layout => self.write(" class=\"map\""),
            _  => {}
        }
        if !id.is_empty() {
            self.write(&format!(" id=\"{id}\""));
        }
        self.write(">");
        if !title.is_empty() && (kind==TableKind::Rifmux || kind==TableKind::Page) {
            self.write("<caption>");
            self.write(&title.to_casing(Casing::Title));
            if kind==TableKind::Page {
                self.write(" Summary");
            }
            self.write("</caption>");
        }
        self.write("\n");
        // For layout table add an empty row above
        if kind==TableKind::Layout {
            self.write("  <tr><td width=\"*\" class=\"map\"></td>\n");
            self.write(&"<td width=\"22\" class=\"map\"></td>".repeat(32));
            self.write("</tr>\n");
        }
    }

    fn write_table_footer(&mut self, _kind: TableKind) {
        self.write("</table>\n");
    }

    fn write_table_row_header(&mut self, id: Option<&str>) {
        self.write("   <tr");
        if let Some(id) = id {
            self.write(&format!(" id=\"{id}\""));
        }
        self.write(">");
    }

    fn write_table_row_footer(&mut self) {
        self.write("</tr>\n")
    }

    /// Write a table cell header
    fn write_table_cell_top(&mut self, kind: (TableKind, CellKind), _span: usize, txt: &str, _id: &str) {
        self.write("<th");
        if kind.0 == TableKind::RegInst {
            if kind.1 == CellKind::Inst {
                self.write(" width=\"33%\"");
            }
            self.write(" class=\"noborders\"");
        }
        self.write(&format!(">{txt}</th>"));
    }

    fn write_table_cell(&mut self, kind: (TableKind, CellKind), span: usize, txt: &str, id: &str, tip: Option<String>) {
        self.write("<td");
        match kind.0 {
            TableKind::RegInst => self.write(" class=\"noborders\""),
            TableKind::Layout => {
                if kind.1==CellKind::Field && txt.is_empty() {
                    self.write(" class=\"rsvd\"");
                } else if kind.1==CellKind::Field && span * 3 * self.nb_col <= txt.len()  {
                    self.write(" class=\"mapv\"");
                } else {
                    self.write(" class=\"map\"");
                }
            }
            _ => {},
        }
        if self.nb_col>1 || span > 1 {
            self.write(&format!(" colspan=\"{}\"", span * self.nb_col));
        }
        if let Some(tip) = tip {
            self.write(&format!(" title=\"{tip}\""));
        }
        self.write(">");
        // Add link to type cell only
        if kind.1==CellKind::Inst && (kind.0==TableKind::Rifmux || kind.0==TableKind::Page) && !id.is_empty() {
            self.write(&format!("<a href=\"#{id}\">{txt}</a>"));
        } else {
            self.write(txt);
        }
        self.write("</td>");
    }

    fn enum_def_desc(&mut self, def: &EnumDef) -> String {
        let mut desc = String::with_capacity(def.len() * 16);
        desc.push_str("<table class=\"noborders\">\n");
        for entry in def.values.iter() {
            let entry_name = self.sanitize(&entry.name);
            let entry_desc = self.sanitize(entry.description.get());
            desc.push_str(&format!("   <tr><td class=\"enum-name\">{}&nbsp;({entry_name})</td>", entry.value));
            desc.push_str(&format!("<td width=\"*\" class=\"enum-desc\">{entry_desc}</td></tr>\n"));
        }
        desc.push_str("</table>\n");
        desc
    }

    fn sanitize(&self, raw: &str) -> String {
        let mut txt = String::with_capacity(raw.len());
        let mut last_char = '-';
        for line in raw.split('\n') {
            let l = line.trim();
            // Insert a line return after each line
            if !txt.is_empty() {
                let need_br = ['.', ':'].iter().any(|c| last_char==*c) || l.chars().next().unwrap_or('a').is_uppercase();
                txt.push_str(if need_br {"<br/>"} else {" "});
            }
            // Replace starting indentation by &nbsp; to
            let nb_spc = line.chars().take_while(|c| c.is_whitespace()).count();
            txt.push_str(&"&nbsp;".repeat(nb_spc));
            //
            last_char = l.chars().last().unwrap_or('.');
            txt.push_str(l);
        }
        txt
    }

    fn write_info(&mut self, info: &str) {
        self.write("<span><p>");
        self.write(info);
        self.write("</p></span>\n");
    }

    /// Add a link to an ID of the document
    fn add_link(&mut self, kind: LinkKind, id: &str) {
        self.write(&format!("&nbsp;&nbsp;&nbsp;&nbsp;<a href=\"#{id}\">{kind}</a>"));
    }
}