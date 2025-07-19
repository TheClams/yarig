# Change Log

## [0.15.0] - Unreleased

### Added
  - Syntax: register/field array index now support comma separated and/or range index (like `[0:3,7]`)
  - Generics: support generics for optional register instance
  - Doc Trait: if a field in an array is disabled for some indexes, there is now added description about it.
  - Hardware Trait: add option const_reg and const_field to add some constants in the package
    such as address, field reset/position

### Fixed
  - Generics: the address error signal was inverted

## [0.14.1] - 2025-07-18

### Fixed
  - Hardware: Generic was not use in the port declaration

### Added
  - Documentation on all diferent target output

## [0.14.0] - 2025-07-17

### Added
  - Support generics in array size instance
  - Generics definition now supports an optional description
  - Add new command-line argument "--check" to only run compilation (no generator called)

### Fixed
  - Override: for register array, indexed and default override are now merged (previously default was fully ignored even without indexed override)
  - Hardware:
    * Handle case of field ro, with hardware rw without any write modifier (i.e. direct feedback of hardware of hardware)
    * Handle limit in register arrays
    * Fix missing check signal declaration when limit is part of field override only

## [0.13.2] - 2025-07-04

### Fixed
  - VHDL: fix definition of arrays (need to be at the end, after struct definitions) and type of local register array


## [0.13.1] - 2025-07-01

### Added
  - Check enum value are unique and fit inside the field

### Fixed
  - Latex: changed reference name of field array to avoid handling of special character []
  - Doc Trait: fix missing suffix of interrupt register reference label

## [0.13.0] - 2025-06-24

### Added
  - Enum can now have an optional floating point representation (e.g. `- <name> = <val_int> (<repr_f64>) "desc"`)
    Python class use it to provide conversion to/from float.
  - Enum label now supported as reset value
  - Python : field representation has three format: `enum (int)` for enum kind, or `float (int)` for field with fractional bits or simply `int`

### Internals
  - Component enum now box the instances (RifInst/RifMuxInst): avoid large allocation stack.

## [0.12.2] - 2025-06-20

### Changed
  - Add check on overlapping register
  - Check field declaration syntax is correct
  - Check rifinstance address is in range of the rifmux address width

### Bug Fixes
  - Latex: fix handling of equations in descriptions
    * First character after a ^ or _ was deleted ...
    * Equation is now properly enclosed between $$ without any character escape

## [0.12.0] - 2025-06-15

### Added
  - Latex equation in description converted to MathML for HTML output

## [0.11.0] - 2025-05-25

### Added
  - Add parser for logical expression: this allows translation from verilog logical expression to VHDL seamlessly.
    Applies to register/field clear, and HwSet/HwClr/HwTgl/Lock

### Bug Fixes
  - HDL: Cleanup some remaining hard-coded rif interface field access
    (now uses proper ExprId so that it works in all HDL generator)
  - Fix clear generation: field setting now has precedence over register setting

### Bug Fixes
  - HDL: fix case where the decode signal was missing

## [0.10.2] - 2025-05-21

### Bug Fixes
  - HDL: fix case where the decode signal was missing

### Changed
  - RAL: use more accurate field access (clear on read, toggle, ...)

## [0.10.1] - 2025-05-06

### Bug Fixes
  - IpXact target: now passes validation at least for local RIF examples
    * Missing conversion to target IpXact
    * Fix various format changes for version 2022

## [0.10.0] - 2025-04-29

### Added
  - Description: support for private description.
    * Syntax is `description.hidden`
    * Description structure is now composed of two strings (public and private) which are concatenated when privacy is private.
      For public privacy on the public part of the description is returned
  - New setting local: When a file is included and its target must be generated (through the use of gen_inc setting)
    it is now possible to generate it relative to its RIF file rather the top file which included it.
    The local setting can be set at top and/or overridden directly in the target specific section.

### Changed
  - The get_output_path function now takes target rather than array of string as input.
    Old function is still available and was renamed as get_output_path_kd.
  - GeneratoreCore now include some basic RIF information available to all generator (like name, address/data width)

## [0.9.4] - 2025-04-23

### Bug Fixes
  - Trait HW:
    * Fix a few legacy usage of full path (if_rif.en) instead of proper LogicExpr
    * Fix rifmux file extension (was hard-coded to sv)
    * Fix RIF package filename call not properly flagged as package
    * Add new LogicExpr::CastFrom when an enum is used as bit vector (explicit conversion required in VHDL)
  - Fixes VHDL output: missing cast around enum, missing component declaration, RIFmux support, ...

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
