# YARIG: Yet Another Register Interface Generator

YARIG is code generation tools to describe register interface of an IP or ASIC/FPGA design.

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

Highlighting for SublimeText is available on [github](https://github.com/TheClams/rif).


## Configuration
 A configuration file allows to specify many options of the generators like the rif file, a list of target, the output path for each targets, ...

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

All settings from the configuration file can be overriden by command-line arguments. Run `yarig -h` to list all available options.

## Generator Traits

To streamline the developement of new output targets, the library contains three base generator traits,
which basically go through the different elements of the structure and apply the visibility checks (public/private).
The exact behavior of each traits can be tweaked with some associated constant.

### Software

### Hardware

### Documentation

---
# TODO

## Feature
 - [ ] Improve nb_frac support:
   - [ ] Support reset value in floating-point
   - [x] Use floating-point value in documentation when nb_frac is non-zero (and use a tool-tip in html to display corresponding integer value)
 - [ ] Support pipe option RTL
 - [ ] Support option to repeat field description for interrupt derived register
 - [ ] Support latex equation in description
 - [ ] Implement a TUI ? (ratatui)
 - [ ] Support overlapping register in hardware: need to check exclusive access (RO/WO)
 - [ ] Check enum size matches the field size
 - [ ] Option to add representation value for enum. Syntax could be 'NAME = VAL (repr) "description"'. Could be usefull when enum is representing a limited set of integer or real values.
 - [ ] Option to control if clear works without clock enable or not

## Documentation
 - [ ] Config file: full description
 - [ ] Base generator trait
 - [ ] Why Yarig vs other existing solution (mostly SystemRDL)
 - [ ] List of syntax example for typical use-cases

## Known Bugs / Edge cases
 - [ ] Support partial fields arrays
 - [ ] Handle counter larger than register size (TBD if this should simply be forbidden or properly handled ...)
 - [ ] Add parsing of LogicExpr (currently logic expression works only for SystemVerilog)
 - [ ] Add check on password not being partial fields
