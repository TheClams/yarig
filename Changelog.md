# Changelog


## [0.5.0] - 2025-02-23
### Added
  - target py: generator for python class
  - configuration using toml file

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
