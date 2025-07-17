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

A default CSS is embedded in the HTML but an external CSS file can be provided (`css` option) to change the document style.

---
## AsciiDoc
The AsciiDoc format is similar to HTML.

The `split` option allows to generate one file per peripheral instead of a grouping all peripherals.

---
## Framemaker (MIF)
The MIF format is similar to HTML.

---
## LaTeX

The LaTeX output is similar to HTML

It has labels for each register and field register.
To easily reference them, here is an example of command that add hyperlink and proper formatting.

```
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
