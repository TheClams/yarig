# YARIG: Yet Another Register Interface Generator

YARIG is code generation tools to describe register interface of an IP or ASIC/FPGA design.

This allows to have one common source to describes registers for all the different views:
 - Hardware: for the actual hardware implementation (SystemVerilog and VHDL)
 - Software: for interacting with the register such a C header, UVM RAL, python classes, ...
 - Documentation: for readable description (HTML/Latex/...)

## Description Language
YARIG uses its own file format named `.rif` to describe the registers.

The objectives when designing the language were:
 - Must be easy to read and write
 - Simple registers (read/write or read-only) should be described in a single line
 - Offers fine control on the generated hardware
 - Allow re-use and hierarchical description

### Syntax example


---
# Syntax


---
# TODO

## Generators
 - [ ] Implement base generator for documentation (from html) :
   - [ ] html: view with a sidebar showing the hierarchy.
   		Could be the basis for a GUI ?
   - [ ] latex
   - [ ] mif
   - [ ] json : both flat (muli file) and hierarchical
   - [ ] svd
   - [ ] IP-XACT
 - [ ] Implement base generator for software (from C) :
   - [ ] python: single file flat (from rifgen)
   - [ ] python: hierarchical
 - [ ] Implement base generator for hardware (from SV):
   - [ ] VHDL

## Feature
 - [ ] Support frac property in field (number of fractionnal bits)
 - [ ] Support $f inside description to display format u7.0 or s0.4
 - [ ] Support pipe option RTL
 - [ ] Support option to repeat field description for interrupt derived register
 - [ ] Support JSON config file: top entry is a target name
 - [ ] Support latex equation in description
 - [ ] Implement a TUI ? (ratatui)
 - [ ] Implement a GUI ? (HTML based)
 - [ ] Support overlapping register in hardware: need to check exclusive access (RO/WO)
 - [ ] Check enum size fit the field size
 - [ ] Option to add representation value for enum. Syntax could 'NAME = VAL (repr) "description"'
 - [ ] Option to control if clear works without clock enable or not

## Known Bugs / Edge cases
 - [ ] Support partial fields arrays
