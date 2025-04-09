use crate::{
    cfg::CfgIpXact,
    comp::comp_inst::{RifFieldInst, RifInst, RifPageInst, RifRegInst},
    rifgen::{Description, EnumDef, FieldSwKind}
};

use super::{
    gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore, InstDict},
    trait_sw::GeneratorSw
};

use yarig_macro::add_gen_core;

#[add_gen_core("ipxact.xml")]
pub struct GeneratorIpXact {
    pub vendor: String,
    pub version: String,
    pub library: String,
    pub data_width: u8,
    pub addr_width: u8,
    pub page_range: Vec<u64>,
}

impl GeneratorIpXact {

    pub fn new(setting: GeneratorBaseSetting, cfg: CfgIpXact) -> Self {
        GeneratorIpXact{
            core: GeneratorCore::new(1,setting),
            vendor : cfg.vendor.map_or("Unknown".to_owned(), |s| s),
            version : cfg.version.map_or("1.0".to_owned(), |s| s),
            library : cfg.library.map_or("IP".to_owned(), |s| s),
            page_range: Vec::new(),
            data_width: 32,
            addr_width: 8,
        }
    }

    fn desc_to_string(&mut self, desc: &Description) -> String {
        if desc.is_empty() {
            return "-".to_owned();
        }
        desc.get()
            .replace("\n","\\n")
            .replace("&","&amp;")
            .replace("<","&lt;")
            .replace(">","&gt;")
    }

}

impl GeneratorSw for GeneratorIpXact {
    const HAS_ENUM        : bool = false;
    const SINGLE_FILE     : bool = false;
    const HAS_UNUSED      : bool = false;
    const INST_BY_PAGE    : bool = true;
    const INC_PAGENAME    : bool = false;
    const INST_ARRAY      : bool = false;
    const IS_HIERARCHICAL : bool = false;
    const HAS_REG_DECL    : bool = false;

    fn write_rif_header(&mut self, rif: &RifInst, _base_addr: Option<u64>) {
        self.data_width = rif.data_width;
        self.addr_width = rif.addr_width;
        self.page_range = rif.pages.iter().rev()
            .scan(1<<self.addr_width, |p,e| {
                let r = *p - e.addr;
                *p = e.addr;
                Some(r)
            }).collect();
        self.write("<ipxact:component\n");
        self.write("   xmlns:ipxact=\"http://www.accellera.org/XMLSchema/IPXACT/1685-2022\"\n");
        self.write("   xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"\n");
        self.write("   xsi:schemaLocation=\"http://www.accellera.org/XMLSchema/IPXACT/1685-2022 http://www.accellera.org/XMLSchema/IPXACT/1685-2022/index.xsd\">\n");
        self.write(&format!("  <ipxact:vendor>{}</ipxact:vendor>\n", self.vendor));
        self.write(&format!("  <ipxact:library>{}</ipxact:library>\n", self.library));
        self.write(&format!("  <ipxact:name>{}</ipxact:name>\n", rif.inst_name));
        self.write(&format!("  <ipxact:version>{}</ipxact:version>\n", self.version));
        // TODO: add bus interface information
        self.write("  <ipxact:memoryMaps>\n");
        self.write("    <ipxact:memoryMap>\n");
        self.write("      <ipxact:name>RegisterMap</ipxact:name>\n");

    }

    fn write_rif_footer(&mut self) {
        self.write("    </ipxact:memoryMap>\n");
        self.write("  </ipxact:memoryMaps>\n");
        self.write("</ipxact:component>\n");
    }

    fn write_page_header(&mut self, name: &str, page: &RifPageInst) {
        let tab = "      ";
        let range = self.page_range.pop().unwrap_or(1<<self.addr_width);
        self.write(&format!("{tab}<ipxact:addressBlock>\n"));
        self.write(&format!("{tab}  <ipxact:name>{name}</ipxact:name>\n"));
        self.write(&format!("{tab}  <ipxact:baseAddress>'h{:x}</ipxact:baseAddress>\n", page.addr));
        self.write(&format!("{tab}  <ipxact:range>'h{range:x}</ipxact:range>\n"));
        self.write(&format!("{tab}  <ipxact:width>{}</ipxact:width>\n", self.data_width));
        self.write(&format!("{tab}  <ipxact:access>read-write</ipxact:access>\n"));

    }

    fn write_reginst(&mut self, _basename: &str, page: &RifPageInst, reg: &RifRegInst, _inst_dict: &InstDict, _is_last: bool) {
        let reg_name = self.casing(&reg.name());
        let desc = self.desc_to_string(&reg.description);
        let addr = page.addr + reg.addr;
        let tab = "        ";
        self.write(&format!("{tab}<ipxact:register>\n"));
        self.write(&format!("{tab}  <ipxact:name>{reg_name}</ipxact:name>\n"));
        self.write(&format!("{tab}  <ipxact:description>{desc}</ipxact:description>\n"));
        self.write(&format!("{tab}  <ipxact:addressOffset>'h{addr:x}</ipxact:addressOffset>\n"));
        self.write(&format!("{tab}  <ipxact:size>{}</ipxact:size>\n", self.data_width));
    }

    fn write_field_decl(&mut self, _basename: &str, _reg: &RifRegInst, field: &RifFieldInst, enum_def: Option<&EnumDef>, _is_last: bool) {
        let tab = "          ".repeat(6*2);
        let name = self.casing(&field.name_flat());
        let desc = self.desc_to_string(&field.description);

        let access = match field.sw_kind {
            FieldSwKind::ReadOnly      |
            FieldSwKind::W1Pulse(_, _) |
            FieldSwKind::Password(_)   |
            FieldSwKind::ReadClr       => "read-only",
            FieldSwKind::WriteOnly => "write-only",
            _ => "read-write",
        };

        self.write(&format!("{tab}<ipxact:field>\n"));
        self.write(&format!("{tab}  <ipxact:name>{name}</ipxact:name>\n"));
        self.write(&format!("{tab}  <ipxact:description>{desc}</ipxact:description>\n"));
        self.write(&format!("{tab}  <ipxact:bitOffset>{}</ipxact:bitOffset>\n", field.lsb));
        self.write(&format!("{tab}  <ipxact:resets>\n"));
        self.write(&format!("{tab}    <ipxact:reset>\n"));
        self.write(&format!("{tab}      <ipxact:value>'h{:x}</ipxact:value>\n",field.reset()));
        self.write(&format!("{tab}    </ipxact:reset>\n"));
        self.write(&format!("{tab}  </ipxact:resets>\n"));
        self.write(&format!("{tab}  <ipxact:bitWidth>{}</ipxact:bitWidth>\n", field.width));
        self.write(&format!("{tab}  <ipxact:access>{access}</ipxact:access>\n"));
        if let Some(enum_def) = enum_def {
            self.write(&format!("{tab}  <ipxact:enumeratedValues>\n"));
            for entry in &enum_def.values {
                self.write(&format!("{tab}    <ipxact:enumeratedValue>\n"));
                let enum_desc = self.desc_to_string(&entry.description);
                self.write(&format!("{tab}      <ipxact:name>{}</ipxact:name>\n", entry.name));
                self.write(&format!("{tab}      <ipxact:description>{enum_desc}</ipxact:description>\n"));
                self.write(&format!("{tab}      <ipxact:value>{}</ipxact:value>\n", entry.value));
                self.write(&format!("{tab}    </ipxact:enumeratedValue>\n"));
            }
            self.write(&format!("{tab}  </ipxact:enumeratedValues>\n"));
        }
        match field.sw_kind {
            FieldSwKind::ReadClr => self.write(&format!("{tab}  <ipxact:readAction>clear</ipxact:readAction>\n")),
            FieldSwKind::W1Clr => self.write(&format!("{tab}  <ipxact:modifiedWriteValues>oneToClear</ipxact:modifiedWriteValues>\n")),
            FieldSwKind::W0Clr => self.write(&format!("{tab}  <ipxact:modifiedWriteValues>zeroToClear</ipxact:modifiedWriteValues>\n")),
            FieldSwKind::W1Set => self.write(&format!("{tab}  <ipxact:modifiedWriteValues>oneToSet</ipxact:modifiedWriteValues>\n")),
            FieldSwKind::W1Tgl => self.write(&format!("{tab}  <ipxact:modifiedWriteValues>oneToToggle</ipxact:modifiedWriteValues>\n")),
            _ => {}
        }
        self.write(&format!("{tab}</ipxact:field>\n"));
    }

    fn write_reg_footer(&mut self, _basename: &str, _reg: &RifRegInst, _is_last: bool) {
        self.write("        </ipxact:register>\n");
    }

    fn write_page_footer(&mut self, _name: &str, _is_last: bool) {
        self.write("      </ipxact:addressBlock>\n");
    }

}
