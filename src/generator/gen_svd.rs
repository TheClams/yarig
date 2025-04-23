use crate::{
    cfg::CfgSvd,
    comp::comp_inst::{RifFieldInst, RifInst, RifPageInst, RifRegInst, RifmuxInst},
    parser::remove_rif,
    rifgen::{Description, EnumDef, FieldSwKind}
};

use super::{
    gen_common::{GeneratorBase, GeneratorBaseSetting, GeneratorCore, InstDict, RifList},
    trait_sw::{GeneratorSw, RifContext}
};

use yarig_macro::add_gen_core;

#[add_gen_core("svd.xml")]
pub struct GeneratorSvd {
    pub vendor: String,
    pub version: String,
    pub rif_top: bool,
    pub data_width: u8,
}

impl GeneratorSvd {

    pub fn new(setting: GeneratorBaseSetting, cfg: CfgSvd) -> Self {
        GeneratorSvd{
            core: GeneratorCore::new(1,setting),
            vendor : cfg.vendor.map_or("Unknown".to_owned(), |s| s),
            version : cfg.version.map_or("1.0".to_owned(), |s| s),
            rif_top: false,
            data_width: 32,
        }
    }

    fn write_svd_header(&mut self, name: &str, desc: &Description, data_width: u8) {
        self.write("<device schemaVersion=\"1.3\" xmlns:xs=\"http://www.w3.org/2001/XMLSchema-instance\" xs:noNamespaceSchemaLocation=\"CMSIS-SVD.xsd\" >\n");
        self.write(&format!("  <vendor>{}</vendor>\n", self.vendor));
        self.write(&format!("  <name>{name}</name>\n"));
        self.write(&format!("  <version>{}</version>\n", self.version));
        let desc = self.desc_to_string(desc);
        self.write(&format!("  <description>{desc}</description>\n"));
        self.write(&format!("  <addressUnitBits>{data_width}</addressUnitBits>\n"));
        self.write(&format!("  <width>{data_width}</width>\n"));
        self.write("  <access>read-write</access>\n");
        self.write("  <peripherals>\n");
    }

    fn write_svd_footer(&mut self) {
        self.write("  </peripherals>\n\n");
        self.write("</device>\n\n");
    }

    fn desc_to_string(&mut self, desc: &Description) -> String {
        if desc.is_empty(self.is_public()) {
            return "-".to_owned();
        }
        desc.get(self.is_public())
            .replace("\n","\\n")
            .replace("&","&amp;")
            .replace("<","&lt;")
            .replace(">","&gt;")
    }

}

impl GeneratorSw for GeneratorSvd {
    const HAS_ENUM        : bool = false;
    const SINGLE_FILE     : bool = true;
    const HAS_UNUSED      : bool = false;
    const INST_BY_PAGE    : bool = false;
    const INC_PAGENAME    : bool = false;
    const INST_ARRAY      : bool = false;
    const IS_HIERARCHICAL : bool = false;
    const HAS_REG_DECL    : bool = false;

    fn write_rif_header(&mut self, rif: &RifInst, base_addr: Option<u64>) {
        if base_addr.is_none() {
            self.write_svd_header(remove_rif(&rif.type_name), &rif.description, rif.data_width);
            self.rif_top = true;
            let cntxt = RifContext {prefix: "", group: "", page: "", addr: 0};
            self.write_rif_inst(rif, cntxt, &rif.description, true, true);
        }
        self.data_width = rif.data_width;
    }

    fn write_rif_footer(&mut self) {
        self.write("    </peripheral>\n");
        if self.rif_top {
            self.write_svd_footer();
        }
    }

    fn write_reginst(&mut self, _basename: &str, page: &RifPageInst, reg: &RifRegInst, _inst_dict: &InstDict, _is_last: bool) {
        let reg_name = self.casing(&reg.name());
        let desc = self.desc_to_string(&reg.description);
        let addr = page.addr + reg.addr;
        let tab = " ".repeat(4*2);
        self.write(&format!("{tab}<register>\n"));
        self.write(&format!("{tab}  <name>{reg_name}</name>\n"));
        self.write(&format!("{tab}  <description>{desc}</description>\n"));
        self.write(&format!("{tab}  <addressOffset>0x{addr:x}</addressOffset>\n"));
        self.write(&format!("{tab}  <size>{}</size>\n", self.data_width));
        self.write(&format!("{tab}  <resetValue>0x{:x}</resetValue>\n", reg.reset));
        self.write(&format!("{tab}  <fields>\n"));
    }

    fn write_field_decl(&mut self, _basename: &str, _reg: &RifRegInst, field: &RifFieldInst, enum_def: Option<&EnumDef>, _is_last: bool) {
        let tab = " ".repeat(6*2);
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

        self.write(&format!("{tab}<field>\n"));
        self.write(&format!("{tab}  <name>{name}</name>\n"));
        self.write(&format!("{tab}  <description>{desc}</description>\n"));
        self.write(&format!("{tab}  <bitOffset>{}</bitOffset>\n", field.lsb));
        self.write(&format!("{tab}  <bitWidth>{}</bitWidth>\n", field.width));
        self.write(&format!("{tab}  <access>{access}</access>\n"));
        match field.sw_kind {
            FieldSwKind::ReadClr => self.write(&format!("{tab}  <readAction>clear</readAction>\n")),
            FieldSwKind::W1Clr => self.write(&format!("{tab}  <modifiedWriteValues>oneToClear</modifiedWriteValues>\n")),
            FieldSwKind::W0Clr => self.write(&format!("{tab}  <modifiedWriteValues>zeroToClear</modifiedWriteValues>\n")),
            FieldSwKind::W1Set => self.write(&format!("{tab}  <modifiedWriteValues>oneToSet</modifiedWriteValues>\n")),
            FieldSwKind::W1Tgl => self.write(&format!("{tab}  <modifiedWriteValues>oneToToggle</modifiedWriteValues>\n")),
            _ => {}
        }

        if let Some(enum_def) = enum_def {
            self.write(&format!("{tab}  <enumeratedValues>\n"));
            for entry in &enum_def.values {
                self.write(&format!("{tab}    <enumeratedValue>\n"));
                let enum_desc = self.desc_to_string(&entry.description);
                self.write(&format!("{tab}      <name>{}</name>\n", entry.name));
                self.write(&format!("{tab}      <description>{enum_desc}</description>\n"));
                self.write(&format!("{tab}      <value>{}</value>\n", entry.value));
                self.write(&format!("{tab}    </enumeratedValue>\n"));
            }
            self.write(&format!("{tab}  </enumeratedValues>\n"));
        }
        self.write(&format!("{tab}</field>\n"));
    }

    fn write_reg_footer(&mut self, _basename: &str, _reg: &RifRegInst, is_last: bool) {
        // Close the fields entry
        self.write("        </fields>\n");
        // Close the register entry
        self.write("      </register>\n");
        if is_last {
            self.write("      </registers>\n");
        }
    }

    fn write_rifmux_header(&mut self, rifmux: &RifmuxInst, _rif_list: &RifList, _rifmux_list: &[&RifmuxInst]) {
        self.write_svd_header(&rifmux.type_name, &rifmux.description, rifmux.data_width);
    }

    fn write_rifmux_footer(&mut self, _rifmux: &RifmuxInst) {
        self.write_svd_footer();
    }

    fn write_rif_inst(&mut self, rif_inst: &RifInst, cntxt: RifContext, desc: &Description, _last_page: bool, _last_comp: bool) {
        let desc = self.desc_to_string(desc);
        let name = self.casing(remove_rif(&rif_inst.type_name));

        self.write(         "    <peripheral>\n");
        self.write(&format!("      <name>{name}</name>\n"));
        self.write(&format!("      <description>{desc}</description>\n"));
        if !cntxt.group.is_empty() {
            self.write(&format!("      <groupName>{}</groupName>\n", cntxt.group));
        }
        self.write(&format!("      <baseAddress>0x{:x}</baseAddress>\n", cntxt.addr));
        self.write(         "      <addressBlock>\n");
        self.write(         "        <offset>0</offset>\n");
        self.write(&format!("        <size>0x{:x}</size>\n", 1<<rif_inst.addr_width));
        self.write(         "        <usage>registers</usage>\n");
        self.write(         "      </addressBlock>\n"       );
        self.write(         "      <registers>\n"       );
    }

}
