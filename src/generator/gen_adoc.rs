use crate::{comp::comp_inst::RifInst, generator::casing::ToCasing, rifgen::EnumDef};

use super::{casing::Casing, gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore}, trait_doc::{CellKind, GeneratorDoc, LinkKind, TableKind}};

#[allow(dead_code)]
pub struct GeneratorAdoc {
    /// Base structure of all generators
    core: GeneratorCore,
    /// Flag when current document is for a RIFMux
    is_rifmux: bool,
    /// Flag when current RIF has multiple pages
    multipage: bool,
}

#[allow(dead_code)]
impl GeneratorAdoc {

    pub fn new(setting: GeneratorBaseSetting) -> Self {
        GeneratorAdoc {
            core: GeneratorCore::new(0,setting),
            is_rifmux: false,
            multipage: false,
        }
    }
}

impl GeneratorBase for GeneratorAdoc {

    const EXT : &'static str = "adoc";

    fn core(&self) -> &GeneratorCore {
        &self.core
    }

    fn core_mut(&mut self) -> &mut GeneratorCore {
        &mut self.core
    }

}

impl GeneratorDoc for GeneratorAdoc {
    const HAS_LAYOUT : bool = false;
    const SHOW_TYPE  : bool = false;
    const SHOW_RESET : bool = false;
    const SHOW_SINGLE_REG : bool = false;
    const SHOW_UNUSED : bool = true;

    fn set_rif_info(&mut self, _addr_w: u8, _data_w: u8, nb_page: usize) {
        self.multipage = nb_page > 1;
    }

    fn write_rif_title(&mut self, idx_rif: (&str,usize), desc: &str) {
        // println!("Title RIF {} : {} | rifmux={}", idx_rif.1, idx_rif.0, self.is_rifmux);
        if idx_rif.1 > 0 && self.is_rifmux {
            self.write("=");
        }
        self.write("= ");
        if idx_rif.1 == 0 || (!self.is_rifmux && idx_rif.1 == 1) {
            self.write("[[top]]");
        }
        self.write(&format!("[[{}]]", idx_rif.0));
        if desc.is_empty() {
            self.write(&idx_rif.0.to_casing(Casing::Title));
        } else {
            self.write(desc);
        }
        self.write("\n\n");
        // Function is first called with index 0 when it is a rifmux
        if idx_rif.1==0 {
            self.is_rifmux = true;
        }
    }

    fn write_page_title(&mut self, idx_rif: (&str,usize), idx_page: (&str, usize), desc: (&str, Option<&str>)) {
        if self.multipage {
            self.write(&format!("\n=== [[{}.{}]]", idx_rif.0, idx_page.0));
            let name = self.sanitize(idx_page.0);
            if desc.0.is_empty() {
                self.write(&name);
            } else {
                self.write(&format!("{} ({name})", desc.0));
            }
        } else if !desc.0.is_empty() {
            self.write(&self.sanitize(desc.0));
            self.write("\n");
        }
        if let Some(desc_detail) = desc.1 {
            self.write_info(&self.sanitize(desc_detail));
            self.write("\n");
        }
    }

    fn write_reg_title(&mut self, idx_rif: (&str,usize), _idx_page: (&str, usize), idx_reg: (&str, usize), desc: &str) {
        self.write("\n");
        if self.is_rifmux {
            self.write("=");
        }
        if self.multipage {
            self.write("=");
        }
        self.write("=== ");
        self.write(&format!("[[{}.{}]]", idx_rif.0, idx_reg.0));
        let name = self.sanitize(idx_reg.0);
        if desc.is_empty() {
            self.write(&name);
        } else {
            self.write(&format!("{desc} ({name})"));
        }
        self.write("\n");
    }

    fn write_reg_summary_header(&mut self, _rif: &RifInst) {
        self.write("\n");
        if self.is_rifmux {
            self.write("=");
        }
        self.write("== Address mapping\n");
    }

    fn write_table_title(&mut self, kind: TableKind, title: &str, id: &str) {
        let caption = if kind==TableKind::Page {
            if self.multipage {
                format!("{} registers address mapping", title.to_casing(Casing::Title))
            } else {
                "Registers address mapping".to_owned()
            }
        } else {
            self.sanitize(title)
        };
        self.write(".");
        self.write(&caption);
        self.write("\n[grid=rows,frame=none]\n");
        self.write("[%header, cols=\"");
        match kind {
            TableKind::Rifmux  => self.write("2,4,9"),
            TableKind::Page    => self.write("2,4,9"),
            TableKind::RegInst => self.write("2,3,2,9"),
            TableKind::Field   => self.write("1,3,1,2,8"),
            _  => self.write("}"),
        }
        self.write("\"]\n");
        if !id.is_empty() {
            self.write(&format!("[[{id}]]\n"));
        }
        self.write("|===\n");
    }

    fn write_table_footer(&mut self, _kind: TableKind) {
        self.write("|===\n\n");
    }

    fn write_reg_detail_header(&mut self, _rif: &RifInst) {
        self.write("\n");
        if self.is_rifmux {
            self.write("=");
        }
        self.write("== Registers definition\n");
    }

    fn write_table_cell(&mut self, kind: (TableKind, CellKind), span: usize, txt: &str, id: &str, _tip: Option<String>) {
        // Call sanitize on non-description cell (already caled on description)
        let txt = if kind.1!=CellKind::Desc {self.sanitize(txt)} else {txt.to_owned()};
        if span > 1 {
            self.write(&format!("{span}+a|{txt}"));
        } else if kind.0==TableKind::FieldRsvd && txt.is_empty() {
            match kind.1 {
                CellKind::Inst => self.write("^|-"),
                CellKind::Desc => self.write("e|Reserved"),
                _s => {}
            }
        } else {
            let has_link = kind.1==CellKind::Inst && !id.is_empty()
                && (kind.0==TableKind::Rifmux || kind.0==TableKind::Page);
            if kind.1 == CellKind::Desc || has_link {
                self.write("a");
            }
            self.write("|");
            if has_link {
                self.write(&format!("<<{id},{txt}>>"));
            } else {
                self.write(&txt);
            }
        }
        self.write("\n");
    }

    fn enum_def_desc(&mut self, def: &EnumDef) -> String {
        let mut desc = String::with_capacity(def.len() * 16);
        desc.push_str("\n\n");
        for entry in &def.values {
            let entry_name = self.sanitize(&entry.name);
            let entry_desc = self.sanitize(entry.description.get());
            desc.push_str(&format!(" * {} - {entry_name} : {entry_desc}\n", entry.value));
        }
        desc
    }

    fn sanitize(&self, raw: &str) -> String {
        let mut txt = String::with_capacity(raw.len());
        for l in raw.split('\n') {
            // Insert a line return after each line
            if !txt.is_empty() {
                txt.push_str(" +\n");
            }
            // Replace starting indentation by non-breaking space
            let nb_spc = l.chars().take_while(|c| c.is_whitespace()).count();
            txt.push_str(&" ".repeat(nb_spc));
            //
            if let Some(note) = l.strip_prefix("Note: ") {
                txt.push_str("\nNOTE: ");
                txt.push_str(note.trim());
            }
            else if let Some(note) = l.strip_prefix("Tip: ") {
                txt.push_str("\nTIP: ");
                txt.push_str(note.trim());
            }
            else if let Some(note) = l.strip_prefix("Important: ") {
                txt.push_str("\nIMPORTANT: ");
                txt.push_str(note.trim());
            }
            else if let Some(note) = l.strip_prefix("Note: ") {
                txt.push_str("\nWARNING: ");
                txt.push_str(note.trim());
            }
            else if let Some(note) = l.strip_prefix("Caution: ") {
                txt.push_str("\nCAUTION: ");
                txt.push_str(note.trim());
            } else {
                txt.push_str(l.trim());
            }
        }
        txt
    }

    fn write_info(&mut self, info: &str) {
        self.write(info);
        self.write("\n\n");
    }

    fn add_link(&mut self, kind: LinkKind, id: &str) {
        match kind {
            LinkKind::Top   => self.write(&format!("<<{id},Back to Top>>\n")),
            LinkKind::Page  => self.write(&format!("<<{id},Back to register summary>>\n")),
            LinkKind::Field => {},
        }
    }

}