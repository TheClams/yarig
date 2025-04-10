# Change Log


## [0.9.2] - 2025-04-13

### Bug Fixes
 - Fix SV package not being generated anymore ...

### Added
 - VHDL Generator

### Changed
 - Update Trait HW: +/- logic expression now include a signed flag

## [0.9.1] - 2025-04-11

### Added
 - Add a register auto instance mode (auto-legacy) where order of the interrupt register is different (mask before enable)

### Bug Fix
 - Python: enumerated was potentially including a package name


## [0.9.0] - 2025-04-11

### Added
 - TraitHw: New generator trait to implement generator for hardware (SV/VHDL)
 - add_gen_core macro (in sub-crate yarig_macro): avoid some boilerplate code for generator:
   It add a core field to a struct and implement the GeneratorBase trait
 - C configuration can override the top gen_inc setting
 - Field limits can now uses parameter
 - Split option for HTML/ADoc/MIF: allow to split rifmux and the RIF output in different files

### Changed
 - Generator SV now use the TraitHw: this comes with significant change on the SV output
    since now all logic expression uses a common type and common output code
 - All generators now uses the new macro

### Internals
 - Add distinct type for resetVal (ReseltValP and ResetVal) before and after "compilation" to ensure all value are fully defined after compilation.

## [0.8.5] - 2025-04-03

### Changed
 - Python:
   * Field can noe return their enum class (function enum_kind())
   * Configuration: gen_inc can be defined in the py parameter to override top gen_inc
   * All field definition now have a docstring
   * Add iterator on rifs inside a peripheral
   * Add name to peripherals
   * Add function to find rif/register/field by name (e.g. my_top.by_name("rif.reg.field"))
   * Move flags inside reginfo and add function to get flags and readonly info for a register

## [0.8.4] - 2025-03-31

### Changed
 - Python: add enum definition and iterator over all registers


## [0.8.3] - 2025-03-19

### Changed
 - Now check field name are not using reserved keyword from SV, and optionnaly VHDL (if it is a target )
   An option allows to automatically rename the field by prepending an underscore.
 - Python: add a peripheral base address to its element
 - Latex: add missing sanitize call for register chapters

## [0.8.2] - 2025-03-14

### Added
 - MIF: options to control paragrah style and table dimension

### Bug Fixes
 - HTML: Fix some missing return line
 - Python: Fix output for version 3.10 (need to skip override decorator, not final)

## [0.8.1] - 2025-03-14

### Added
  - New output target: IP-XACT (still need some validation)
  - HTML: add option to specify an external css file
  - CLI option to specify directory for JSON (previously was using the more genric doc one)

### Changed
  - SW Trait: write_page_footer description argument now replace by reference to page to provide access to all the page properties

## [0.8.0] - 2025-03-13
### Added
  - Add possibility to specify a python version (support 3.10 to 3.12 currently)

### Changed
  - Generator doc (HTML, latex, Mif, adoc): display absolute address when possible
  - SystemVerilog: avoid declaring unused `__decode` signals

### Bug Fixes
  - JSON: add missing double quote in rifmux

### Internal
  - RifList: object now contains the list of all instance of rif with their name and address:

## [0.7.0] - 2025-03-10

### Changed
  - Configuration:
    * Support custom targets (silently ignored in the main)
    * Support override of HDL interface
    * New option "suffix_rtl_only" to add suffix only on RTL file
    * Casing is now configurable
    * Support specifying the filename in the output path for the top target
  - Generator HTML:
    * Enum description can be changed via CSS and by default is displayed with smaller font
    * Field description now has breaking line only after a dot or colon
    * A tooltip is displayed for reset value with a fractional number of bits,
      or when the field is part of a registered instantiated multiple times with different values
  - Syntax:
    * Setting a read access to a hardwrae field which already has write access promote it to RW
    * Alternate interrupt inherit all its property from the main interrupt

### Bug Fixes
  - Generator SV:
    * Fix issue with register declared as an array
    * Fix issue with only one external register inside a group
    * Fix registered pulse using clock enable
    * Change handling of field with clear and clk_en: both condition are split (might need to have an option allowing to have clear gated by the enable)
    * Fix some cases where the gen_include check was more restratictive than in other generators
    * Fix signal naming of alternate interrupt
    * Improve Interrupt clock enable
  - Generator RAL:
    * Fix issue with register arrays
    * Fix handling of multiple pages
    * Fix naming for the rimux top
  - Generator C:
    * add missing include stdint.h
    * add missing define for group address

## [0.6.0] - 2025-03-02
### Added
  - New targets:
    * JSON: a basic representation of register with field position/reset/description and kind (RW/RO/....)
    * AsciiDoctor: doc similar to HTML/latex output
    * SVD (System View Description): standard register file description used in embedded development

### Changed
  - Python and C target now uses the nb_frac property
    * Python: base field functions allows to get the floating point value
    * C: define a macro with number of fractional bits for each field (when not null)
  - Config file:
    * C casing is configurable in the configuration file
    * Support relative file location
    * Output path Can now be overridden by CLI arguments

### Bug Fixes
  - Description interpolation of $i/$f was not supporting multiple variable in a line
  - Register visibility was not respected

### Internal
  - Field structure now has a is_signed() function
  - Trait Sw :
    * All function now have an empty implementation to avoid having to implement them in each targets
    * Add option to skip the register declaration (HAS_REG_DECL)
    * Add is_last parameter to a few function (reg_footer, field_decl, reg_inst, rif_inst, rifmux_inst)
  - Trait doc: add some hook functions before register summary and register detail

## [0.5.0] - 2025-02-23
### Added
  - target py: generator for python class
  - configuration using toml file
  - New field property: `nb_frac` to specify number of fractional bits

### Changed
  - Trait Sw:
    * use RegInst/RifmuxInst in many functions instead of passing some fields
    * Add function allowing to create other file on top of the generated file (like base class in python)

### Internal
  - Update all dependencies (winnow 0.7) and switch to Rust Edition 2024
  - Field struct now has additional helper function for checking its kind (is_pulse, is_special)

## [0.4.0] - 2025-01-19
### Added
  - target ral: generator for systemVerilog Register Abstraction Layer

### Changed
  - Trait Sw:
    * Add a few const to control the generator (instance per arrays or element, hierarchical or flatten)
    * update a fw trait functions prototype to be more generic

## [0.3.3] - 2025-01-19
### Added
  - Trait SW: software trait to use as basis for generator having software access (C, python, ...)
    * C generator now based on this trait

### Changed
  - Parser:
    * Use enum for address width: ensure width is supported, and centralize some feature such as nb_byte, address mask, ...
    * check address alignment vs data width

## [0.3.2] - 2025-01-15
### Changed
  - Error message improved:
    * now contains current file name
    * explicit context name in different parser (might need more details like allowed keywords, ...)

## [0.3.1] - 2025-01-12
Crate is now a library to allow building other applications reusing parser and generators trait
