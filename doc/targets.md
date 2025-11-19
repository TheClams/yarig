# Targets

This page provides information about the different generator outputs (except hardware which has its own [dedicated page](package.md)).

---
## UVM RAL

This target generate one file per peripheral defining register blocks based on the uvm class `uvm_reg_block`,
but it is possible to use a proprietary class as-long as it is derived from uvm_reg_block.

For the rifmux it creates a reg_block instantiating each peripheral at the proper address.
It defines a macro `ral_create_reg_block` to call the configure/build/add_submap functions,
but again it is possible to provide a user-defined macro which has the same kind of parameters:
block_name, block_type, address.

---
## C
For each register file definition the generator outputs a header file containing:

 - One structure per register (union between "uint32 reg32" and a struct of bitfields called fields)
 - Macro definition for each field giving their position and mask
 - One structure per page grouping all register together, and inserting reserved register when needed

The naming convention is:

 - Register struct: RifName + RegisterName + "Reg_u" (using PascalCase)
 - Field name inside register struct uses camelCase
 - Macro: RIFNAME_REGNAME_FIELDNAME + "\_POS"|"\_MASK"|"\_SMASK"
 - RIF struct: RifName + "Regs" (using PascalCase).
 - Register name inside RIF struct uses camelCase

If a rifmux is provided, an additional file is generated including all others header files
and defining macro for base address and pointer to those addresses.

The naming convention is:

 - Base address : RIFINSTNAME_BASE_ADDR
 - Pointer : P_RIFINSTNAME

When there is more than one page in a RIF, the page name is appended to the RIF name.

---
## Python

The generator create one file per peripheral.
A base [python class](../src/generator/resources/regmap.py) provides common API for all peripheral, register and fields.

There are classes defined for each registers and fields, which are then instantiated when an object is created.
So to use the resulting python class simply create an instance of the rifmux `regmap = MyRifmux()`.

The python output has proper typing to provide good auto-complete and documentation through LSP (tested with pyright).

---
## JSON
The JSON output is one file per peripheral with basic information.

Each registers has the following entries:
  - `addr`: address (integer)
  - `desc`: Register description (string)
  - `readOnly`: false/true
  - `flags`: array of special properties such as "interrupt" and "external"
  - `fields`: dictionnary of all fields available in the register

Each fields has the following entries:
  - `pos` : Field position in the register (i.e. lsb)
  - `width` : Number of bits of the fields
  - `value` : reset value
  - `signed`: true/false
  - `kind` : String representing the software access kind (ro, rw, wo, rclr, w1clr, w1set, w1tgl, pulse, password)
  - `desc` : Field description


---
## HTML
The HTML output for a rifmux provides a summary table of all peripheral (name & address)
with link to paragraph providing a list of the registers in the peripheral
which itself links to detailled register description in sub-paragraph.

The `split` option allows to generate one file per peripheral instead of a grouping all peripherals.

### CSS Styling

To customize the appearance of the generated HTML documentation, you can provide your own CSS file using the `css` option. The default CSS file is located at `src/generator/resources/style.css` and can be used as a reference or starting point for your custom stylesheet.

The HTML generator uses the following CSS classes and selectors:

#### Document Structure
- `div.fulldoc` - Main document container that wraps all content
- `h2` - Section headings (peripheral pages)
- `h3` - Subsection headings (register descriptions)

#### Tables
- `table` - Base styling for all tables (register tables, summaries)
- `th` - Table header cells with gray background (#CCCCCC)
- `td` - Table data cells with gray borders
- `caption` - Table captions (appears above tables)

#### Register Layout Tables
- `table.map` - Register bit layout tables with full borders
- `td.map` - Standard cells in layout tables (8pt font, centered)
- `td.mapv` - Vertical text cells for long field names (rotated 90 degrees)
- `td.rsvd` - Reserved/unused bit fields (light gray background #EEEEEE)

#### Register Instance Tables
- `table.noborders` - Tables without borders (used for register instance listings)
- `td.noborders` - Cells without borders

#### Enumeration Descriptions
- `td.enum-name` - Enumeration value and name (5% width, no wrap)
- `td.enum-desc` - Enumeration description (95% width)

#### Notes and Information
- `table.note` - Note/information tables
- `td.noteHeader` - Note header cell (bold, red text #990000, right border)
- `td.noteContent` - Note content cell

#### Conditional Fields
- `tr.cond` - Conditional register rows (yellow background #F0F0C0)
- `#cond` - Conditional elements (nested conditionals use different shades: #FBDFC7, #ABCBCB)

#### Popups
- `div.popup` - Popup container (hidden by default, positioned absolutely)
- `table.popup` - Popup table styling (brown borders #76461f)
- `th.popup` - Popup header (brown background #B6865E)
- `td.popup` - Popup content cell (tan background #DBAF8B)

#### Links
- `a` - All links (pointer cursor)

When creating a custom CSS file, you can override any of these selectors to match your preferred styling. The default CSS uses Arial/Verdana fonts at 10pt for main content and 8pt for layout tables, with gray color scheme (#CCCCCC, gray borders). You can modify colors, fonts, spacing, borders, and any other visual properties to suit your documentation standards.

---
## AsciiDoc
The AsciiDoc format is similar to HTML.

The `split` option allows to generate one file per peripheral instead of a grouping all peripherals.

### Link Format

The AsciiDoc generator creates anchors and links that can be referenced from other AsciiDoc documents. The following link formats are used:

#### Anchors
Anchors are created using the `[[id]]` format:
- `[[top]]` - Anchor at the top of the document
- `[[rifname]]` - Anchor for a RIF/peripheral (where `rifname` is the RIF instance name)
- `[[rifname.pagename]]` - Anchor for a page within a peripheral
- `[[rifname.regname]]` - Anchor for a register (where `regname` is the register name)
- Table anchors use the ID passed to the table generation function

#### Internal Links
Within the same document, links use the `<<id,text>>` format:
- `<<top,Back to Top>>` - Link to the top of the document
- `<<rifname,Peripheral Name>>` - Link to a peripheral section
- `<<rifname.pagename,Page Name>>` - Link to a page section
- `<<rifname.regname,Register Name>>` - Link to a register section

#### Cross-Document Links
When the `split` option is enabled, cross-document references use the `xref:` format:
- `xref:component_name.adoc#rifname[Peripheral Name]` - Link to a peripheral in another document
- `xref:component_name.adoc#rifname.pagename[Page Name]` - Link to a page in another document
- `xref:component_name.adoc#rifname.regname[Register Name]` - Link to a register in another document

Where `component_name` is the RIF component name (without the `.rif` extension).

#### Example Usage
To reference a register from another AsciiDoc document:
```asciidoc
See the xref:spi.adoc#spi.ctrl[SPI Control Register] for details.
```

To reference a register within the same document:
```asciidoc
See the <<spi.ctrl,SPI Control Register>> for details.
```

---
## Framemaker (MIF)
The MIF format is similar to HTML.

The `split` option allows to generate one file per peripheral instead of grouping all peripherals.

### Style Definitions

FrameMaker style definitions can be customized through the configuration file to match your document template. The following style options are available:

#### Paragraph Styles
- `anchor` (default: `"Body"`) - Paragraph style for anchor paragraphs and general information text
- `h2` (default: `"2heading"`) - Paragraph style for level 2 headings, used for peripheral titles, page titles, and "Address Mapping" section headings
- `h3` (default: `"3headingReg"`) - Paragraph style for level 3 headings, used for register titles (includes register name, description, and address)

#### Table Format Paragraph Tags
These define the table format/style applied to entire tables:
- `table_title` (default: `"Mapping Table"`) - Table format paragraph tag for RifMux tables (peripheral summary) and mapping tables (page address mapping)
- `table_heading` (default: `"Mapping Table"`) - Table format paragraph tag for register instance tables
- `table_cell` (default: `"Register"`) - Table format paragraph tag for field definition tables

#### Table Cell Paragraph Tags
These define the paragraph style applied to cells within tables:
- `table_kind_rifmux` (default: `"TableTitle"`) - Paragraph style for table title/caption text (e.g., "Table 1. Registers address mapping")
- `table_kind_mapping` (default: `"CellHeading"`) - Paragraph style for table header cells (column headers)
- `table_kind_reg` (default: `"CellBodyLeft"`) - Paragraph style for table body cells (data cells in register and field tables)

#### Column Width Specifications
These arrays define the column widths in centimeters for different table types:
- `width_3col` (default: `[2.5, 3.4, 11.1]`) - Column widths for 3-column tables: RifMux tables (Offset, Name, Description) and Page mapping tables
- `width_4col` (default: `[2.5, 3.4, 2.1, 9.4]`) - Column widths for 4-column register instance tables (Offset, Name, Reset, Description)
- `width_5col` (default: `[1.4, 3.4, 2.0, 2.1, 8.5]`) - Column widths for 5-column field definition tables (Width, Name, Access, Reset, Description)

All style names must correspond to paragraph formats or table formats defined in your FrameMaker document template. See `doc/config.md` for the complete configuration reference.

---
## LaTeX

The LaTeX output is similar to HTML.

The `split` option allows to generate one file per peripheral instead of grouping all peripherals.

The `casing` option allows to specify the casing convention for register and field names.

It has labels for each register and field register.
To easily reference them, here is an example of command that add hyperlink and proper formatting.

```latex
\makeatletter
\DeclareRobustCommand*{\escapeus}[1]{%
  \begingroup\@activeus\scantokens{#1\endinput}\endgroup}
\begingroup\lccode`\~=`\_\relax
   \lowercase{\endgroup\def\@activeus{\catcode`\_=\active \let~\_}}
\makeatother

% Define command to display registers properly and with a link to the RIF table
\newcounter{rif}
\newcommand{\reg}[1]{\hyperref[rif:rifname.#1]{\texttt{\escapeus{#1}}}}
```

---
## CMSIS-SVD (System View Description)
Standard CMSIS-SVD output file.

The vendor and version properties can be set via the configutation file.

---
## IP-XACT
Standard IP-XACT with one file per peripheral.

The vendor, library and version properties can be set via the configutation file.
