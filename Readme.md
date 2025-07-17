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
it is quite compact when defining simple register, with default behavior that should intuitive.
For example, if there is no reset value an no hardware/software defined, then it is assumed this is a read-only register driven by hardware.

In term of features missing from SystemRDL, the main one I have identified are:
  - Overriding default precedence hw/sw, parity check field
  - Automatic control of endian-ness
  - Definition of HDL path, constraint

But I think RIF has a few improved featured (in no particular order) :
  - RIF mapping: it is possible to defines the whole register map of a SoC by instantiating multiple RIFs.
  - Fine control over both the hardware register structure and its software mapping: instead of defining a whole register structure
    which is then mapped automatically on address aligned with the software bus, the definition of register must be aligned on the software bus,
    but each register can have a group name so that they appear in the same structure in hardware.
  - fine control of clock/reset used per register/field: there is still the hypothesis that all clock are synchronous but it is easy to have fields driven by different clock/reset.
  - pulse field can be defined as combinatorial or register
  - password field: allows to set high an internal signal only when a given value is written.
    This allows to protect some register against write access until the password is written, useful for critical register related to power or clock tree for example.
  - limit property: allows to ensure only a subset of value can be written in a register
  - enum can have a representation value on top of their encoded integer value. (for example `CR_1_2 = 1 (0.5) "Coderate 1/2" `)
  - Defining number of fractional bits associated with a field: allows to define reset value in a more intuitive way when the register represent fixed-point values
  - Control visibility of description: when generating documentation for public purposes, sometimes is is useful to hide some register,
    some part of the description, or automatically rename fields as `reserved_xx` to allow access without exposing the full meaning behind the register


## Configuration
 A configuration file allows to specify many options of the generators like the RIF file, a list of target, the output path for each targets, ...

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

To streamline the development of new output targets, the library contains three base generator traits,
which basically go through the different elements of the structure and apply the visibility checks (public/private).
The exact behavior of each traits can be tweaked with some associated constant.

### Software

### Hardware

### Documentation

---
# TODO

## Feature
  - [ ] Support for generics
    - [x] Add description in generic definition
    - [x] Generics for register instance size
    - [ ] Generics for optional register
    - [ ] Generics for optional pages
    - [ ] Generics for field array: the field size would be defined by the max value (since struct are not parameterizable in SV)
          but the logic for out-of-range field element would be forced to 0
    - [ ] The else part of generic should force unused signal to 0 to avoid lint warning ? (TBC)
  - [x] Support reset value as enum name
  - [ ] Support for AXI4 bus
  - [ ] Support pipe option RTL
  - [ ] Support option to repeat field description for interrupt derived register
  - [x] Support latex equation in description
  - [ ] Support overlapping register in hardware. Exclusive access (RO/WO) is already check at compile time: might be a good place to add some overlap flag to ease the generator job ...
  - [ ] Option to add representation value for enum. Syntax could be 'NAME = VAL (repr) "description"'. Could be useful when enum is representing a limited set of integer or real values.
    - [x] Update parser to support new syntax
    - [x] Add python function in the enum class for conversion to/from float
    - [ ] Add C function for conversion to/from float/int ? Would need to add option to the generator allowing to disable the feature, use int, float or double as the representation type
  - [ ] Option to control if clear works without clock enable or not
  - [ ] Add syntax to support override on a range/list of register/field array. Syntax could be [1:5], or [1,3,7] and ideally support a mix like [0:5,8,11:13,15]
  - [ ] For field array with register array definition, support case where total size is not reg_dim * field_dim. First idea would be to use the field reset initialization: if it's different from 1 and field_dim, then consider this is the total size

## Documentation
  - [x] Config file: full description
  - [ ] Base generator trait
  - [x] Why Yarig vs other existing solution (mostly SystemRDL)
  - [ ] List of syntax example for typical use-cases
  - [ ] Improve targets.md
    - [ ] Detail tag naming in AsciiDoc
    - [ ] Detail CSS property for HTML
    - [ ] Detail styling for Framemaker

## Known Bugs / Edge cases
 - [ ] Support partial fields arrays
 - [ ] Support partial counter (for counter larger than register size)
 - [x] Add parsing of LogicExpr (currently logic expression works only for SystemVerilog)
 - [x] Add check on password not being partial fields
