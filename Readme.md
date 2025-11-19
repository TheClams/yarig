# YARIG: Yet Another Register Interface Generator

YARIG is a code generation tool to describe register interface of an IP or ASIC/FPGA design.

This allows to have one common source to describes registers for all the different views:
 - Hardware: for the actual hardware implementation (SystemVerilog and VHDL)
 - Software: for interacting with the register such a C header, UVM RAL, python classes, ...
 - Documentation: for readable description (HTML/Latex/...)

More details on targets are available in [hardware targets](doc/package.md)
and [other targets](doc/targets.md).

## Description Language
YARIG uses its own file format named `.rif` to describe the registers.

The objectives when designing the language were:
 - Must be easy to read and write
 - Simple registers (read/write or read-only) should be described in a single line
 - Offers fine control on the generated hardware
 - Allow re-use and hierarchical description

Complete details are available in the [syntax page](doc/syntax.md)

### Syntax example

Here is a simple RIF definition:
```
rif: test_rif
  addrWidth: 8
  dataWidth: 16
  - Main:
    registers:
      - ctrl: "Basic control register"
        - en      = 0    0:0        "Block enable"
        - start   = 0    0:0  pulse "Start block"
        - version = 0x12 15:8 ro    "Block version"
          hw na
      - interrupt: "Interrupt register"
        interrupt rising en=0x13 mask=0x37 pending w1clr
        enable.description "Enable interrupt"
        mask.description "Mask interrupt"
        pending.description "Pending interrupt"
        - gen   = 0 7:0 rw "Generic Events"
        - busy  = 0 8:8 "Busy"
        - ready = 0 9:9 "Ready"
      - status: "Status register"
        - state  3:0 "Current state"
        - success 4:4 "Last operation succeed"
        - failed  5:5 "Last operation failed"
    instances: auto
```

Highlighting for SublimeText is available on [github](https://github.com/TheClams/rif)
and for VScode on the [marketplace](https://marketplace.visualstudio.com/items?itemName=Clamsax.rifgen-syntax)

### Why another language ?

Of all existing approach, I think that only SystemRDL comes close to covering all the features I need.
The idea of separating register definition and instance is key to allow re-use among different project.
But the main weakness of SystemRDL (in my opinion) is that the syntax is neither easy to read nor easy to write.

How simpler is the RIF syntax compare to the SystemRDL is quite subjective, but like can be seen in the syntax example
it is quite compact when defining simple register, with default behavior that should feel intuitive.
For example, if there is no reset value an no hardware/software defined, then it is assumed this is a read-only register driven by hardware.

In term of features missing from SystemRDL, the main one I have identified are:
  - Overriding default precedence hw/sw, parity check field
  - Automatic control of endian-ness
  - Definition of HDL path, constraint

But I think RIF has a few improved featured (in no particular order) :
  - Fine control over both the hardware register structure and its software mapping: instead of defining a whole register structure
    which is then mapped automatically on address aligned with the software bus, the definition of register must be aligned on the software bus,
    but each register can have a group name so that they appear in the same structure in hardware.
  - Fine control of clock/reset used per register/field: there is still the hypothesis that all clock are synchronous but it is easy to have fields driven by different clock/reset.
  - Pulse field can be defined as combinatorial or register
  - Password field: allows to set high an internal signal only when a given value is written.
    This allows to protect some register against write access until the password is written, useful for critical register related to power or clock tree for example.
  - Limit property: allows to ensure only a subset of value can be written in a register
  - Enum can have a representation value on top of their encoded integer value. (for example `CR_1_2 = 1 (0.5) "Coderate 1/2" `)
  - Defining number of fractional bits associated with a field: allows to define reset value in a more intuitive way when the register represent fixed-point values
  - Visibility control: when generating documentation for public purposes, sometimes is is useful to hide some register,
    some part of the description, or automatically rename fields as `reserved_xx` to allow access without exposing the full meaning behind the register
  - Generics, i.e. HDL compile-time parameter


## Configuration
A configuration file allows to specify many options of the generators like the RIF file, a list of target, the output path for each targets, ...

All available options are described in the [config page](doc/config.md).

Here is an example:
```
filename = "my_chip.rif"
gen_inc = ["*"]
targets = ["py", "sv", "html", "latex", "c"]

[suffixes.spi]
name = "full"
pkg = true

[outputs]
doc = "../yarig/doc"
c = "../yarig/c"
py = "../yarig/py"
rtl = "../yarig/rtl"
sim = "../yarig/sim"
vhdl = "../yarig/vhdl"
```

Most settings from the configuration file can be overridden by command-line arguments. Run `yarig -h` to list all available options.

## Generator Traits

To streamline the development of new output targets, the library contains three generator traits,
which basically go through the different elements of the structure and apply the visibility checks (public/private).
The exact behavior of each traits can be tweaked with some associated constants/functions.

### Base
All generator are based on the struct GeneratorCore and trait GeneratorBase.

The core offers some basic information/features needed by most generator:
  - a set of common setting (output file name, casing, privacy level, split mode, list of include to generate)
  - the current component information (name, rifmux/rif, address/data width, a page/group counter)
  - A text buffer and a set of n stash allowing to separate the build order from the final output result.
    A set of function allow to manipulate stashes like:
    * `push_stash` to add string to a stash
    * `pop_stash` to move a stash content to the main buffer
    * `pop_stash_to` to move a stash content to another stash
  - A save function which automatically create the whole directory hierarchy (if needed),
    write the text buffer content to the file and clear all buffers
  - A function to get a field name respecting both settings of privacy and casing

The GeneratorBase trait is required by all three specialized traits (Hardware/Software/Documentation)
and basically gives access to the GeneratorCore features.
It has an associated constant to define the file extension wanted and provides two functions (filename_rif/rifmux)
to get consistent output file naming (when no name is provided by the user).

To avoid boilerplate when defining a new generator, the derive macro `add_gen_core("ext")`
automatically add a field `core: GeneratorCore` to a structure and implements the GeneratorBase trait for this struct.

All generators are based on one of the three available traits and can serve as good example of how to develop a new generator.
Code is located in src/generator.

### Software
This trait creates a view of the components from the software point of view.

It is highly configurable and supports both typed and untyped output:
  - typed output typically have first a section declaring a type for each register and enum kind, followed by instances
  - untyped output are simple list of component/register/fields

The following associated constant allows to tweak the generated output:
  - `HAS_ENUM` : Declare enum type
  - `SINGLE_FILE` : Generate a single file for the rifmux and all its RIF definition
  - `HAS_UNUSED` : Unused part of the register also have field declaration
  - `INST_BY_PAGE` : Create one structure per page instead of grouping all register and ignoring the page structure
  - `INC_PAGENAME` : Include pagename inside basename of registers
  - `INST_ARRAY` : Create only one register instance per array (false to create instance per element)
  - `IS_HIERARCHICAL` : Instantiate rifmux instead of flatenning all rifs instance
  - `HAS_REG_DECL` : Create register type declaration before register instance

This trait is used by the generators for C, IPXact, JSON, Python, UVM RAL and SVD.

### Hardware

This trait creates a view of the components from the hardware point of view.

It creates a package for type/enum/constant definition and a
second file containing the actual hardware implementation.

For RIF component, the following constant are declared
  - Address and data width for the software interface
  - If the function `has_const_reg()` is true: address and reset value of all registers
  - If the function `has_const_field()` is true: mask, LSB/MSB and reset value of each fields

For RIFMux, he constant in the package are the address of all RIF components.

The following associated constant allows to tweak the generated output:
  - `SUPPORT_INTF`      : Support SystemVerilog like interfaces
  - `SUPPORT_IMPL_BIND` : Support implicit binding of ports (`.*` in SystemVerilog)

This trait is used by the generators for SV and VHDL.

### Documentation
This trait shows a component description with multiple table with links:
  - A RifMux starts with a table containing all the component instances, followed by one paragraph per component type.
    Each instance includes a link to the corresponding component type
  - A Rif starts with a table summary of all register instance followed by one sub-paragraph per register
  - The register description is composed of up to three tables:
    * If there is more than one instance of this register type,
      a list of all register instances with their reset value and description
    * A register layout showing a quick overview of the field position in the register, along with their default value
    * A table of all fields with the position, description amd reset value

The following associated constant allows to tweak the generated output:
  - `HAS_LAYOUT` : Display register layout table
  - `SHOW_RESET` : Show reset in the register summary
  - `SHOW_TYPE`  : Show type name in both summary tables (top/register)
  - `SHOW_SINGLE_REG` : Show register instance table even when there is only one instance
  - `SHOW_UNUSED` : Unused part of a register are displayed as reserved in the field table

This trait is used by the generators for AsciiDoc, HTML, Latex and Mif (FrameMaker).

---
# TODO

## Feature
  - [ ] Support for generics
    - [x] Add description in generic definition
    - [x] Generics for register instance size
    - [x] Generics for optional register
    - [ ] Generics for optional pages
    - [ ] Generics for optional fields
    - [ ] Generics for field array: the field size would be defined by the max value (since struct are not parameterizable in SV)
          but the logic for out-of-range field element would be forced to 0
    - [ ] The else part of generic should force unused signal to 0 to avoid lint warning ? (TBC)
    - [ ] Value used in documentation should be controllable via parameter settings (default to max, like currently)
  - [x] Support reset value as enum name
  - [ ] Support for AXI4 bus
  - [ ] Support pipe option RTL
    - [x] No pipe level
    - [ ] Two pipe level (pipe on read_data)
  - [ ] Support option to repeat field description for interrupt derived register
  - [x] Support latex equation in description
  - [ ] Support overlapping register in hardware. Exclusive access (RO/WO) is already check at compile time: might be a good place to add some overlap flag to ease the generator job ...
  - [ ] Option to add representation value for enum. Syntax could be 'NAME = VAL (repr) "description"'. Could be useful when enum is representing a limited set of integer or real values.
    - [x] Update parser to support new syntax
    - [x] Add python function in the enum class for conversion to/from float
    - [ ] Add C function for conversion to/from float/int ? Would need to add option to the generator allowing to disable the feature, use int, float or double as the representation type
  - [ ] Option to control if clear works without clock enable or not
  - [x] Add syntax to support override on a range/list of register/field array. Syntax could be [1:5], or [1,3,7] and ideally support a mix like [0:5,8,11:13,15]
  - [x] For field array with register array definition, support case where total size is not reg_dim * field_dim.
        First idea would be to use the field reset initialization: if it's different from 1 and field_dim, then consider this is the total size
  - [ ] SV Generator: use SV interface for non-default case with option to control the interface type name
  - [x] Add some information in generated documentation for disabled field inside an array
  - [x] Hardware generator: support option to add some constants in the package such as address, field reset/position
  - [ ] Support byte mask on interface (TBD if really needed)

## Documentation
  - [x] Config file: full description
  - [x] Base generator trait
    - [x] Core
    - [x] Hardware
    - [x] Software
    - [x] Documentation
  - [x] Why Yarig vs other existing solution (mostly SystemRDL)
  - [ ] List of syntax example for typical use-cases
  - [x] Improve targets.md
    - [x] Detail tag naming in AsciiDoc
    - [x] Detail CSS property for HTML
    - [x] Detail styling for Framemaker

## Known Bugs / Edge cases
 - [ ] Support partial fields arrays
 - [ ] Support partial counter (for counter larger than register size)
 - [x] Add parsing of LogicExpr (currently logic expression works only for SystemVerilog)
 - [x] Add check on password not being partial fields
 - [ ] Add check that reset value fit the field width
