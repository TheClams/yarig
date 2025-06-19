# Configuration File Documentation

The RIF generator uses a TOML configuration file to control various aspects of documentation/code generation.

## Basic Configuration

### Top-Level Options

```toml
# Required: Path to the RIF file to compile
filename = "my_chip.rif"

# Optional: Reference path for relative paths (auto-detected from config file location)
path = "/path/to/project"

# Optional: List of include paths for referenced RIF files
include = ["./include", "../common"]

# Optional: List of included references to generate (use ["*"] for all)
gen_inc = ["*"]

# Optional: List of included references to generate locally,
# i.e. using the RIF file location as the reference for relative path
# (use ["*"] to match all components in gen_inc)
local = ["module1", "module2"]

# Required: List of target formats to generate
targets = ["sv", "c", "html", "py", "ral"]

# Optional: Generate only public registers/fields (hide private ones)
public = false

# Optional: Override the HDL interface type
# Valid values: "default", "apb", "uaux"
interface = "apb"

# Optional: Use suffixes only for RTL generation targets
suffix_rtl_only = false
```

### Parameters

Override parameter value defined in RIF files:

```toml
[parameters]
DATA_WIDTH = 32
ADDR_WIDTH = 16
NUM_CHANNELS = 4
```

### Output Paths

Specify where generated files should be placed.

There are a few fallback target:
 - `doc` correspond to any document generator (htnl, latex, adoc, mif, json)
 - `sw` correspond to any software generator (c, python, svd, ipxact)
 - `rtl` correspond to any hardware generator (sv, vhdl)
 - `sim` correspond to any simulation generator (ral)

```toml
[outputs]
# Documentation outputs
doc = "./doc"
html = "./doc/html"
latex = "./doc/latex"
adoc = "./doc/adoc"
mif = "./doc/mif"
json = "./doc/json"

# Software outputs
c = "./sw/include"
sw = "./sw"
py = "./python"
svd = "./sw/svd"
ipxact = "./sw/ipxact"

# Hardware outputs
rtl = "./rtl"
sv = "./rtl/sv"
vhdl = "./rtl/vhdl"

# Simulation outputs
sim = "./sim"
ral = "./sim/ral"

# Custom target outputs
custom_target = "./custom"
```

### Suffixes

When generating RIF using parameters, it is sometimes useful to add suffix to the generated file.

The syntax to set the suffix of a rif is to defined a new table named `[suffixes.<rif_name>]` and provides at least a name.
There are two optional key/value boolean pairs, default to false:
 - `pkg` : Suffix apply to the generate package file as well
 - `alt_pos` : Place suffix after the \_pkg instead of before when generating package

```toml
[suffixes.spi]
name = "full"
pkg = true
alt_pos = true

[suffixes.uart]
name = "lite"
pkg = false
```

### Casing

It is possible to specify the casing use in regiater/field name either globaly or for each generator (if supported by the generator).

The casing options supported are :
 - `raw`: as-is, casing is not modified
 - `snake` : snake_case
 - `pascal`: PascalCase
 - `camel`: camelCase
 - `kebab`: kebab-case
 - `title`: Title Case

# Optional: Specify casing convention for generated code
# Valid values depend on target language
casing = "snake"

### Reserved keywords handling

The parser can check that field name are not using keyword of certain language.
For the moment it support only a check on common keyword from SystemVerilog and VHDL which could be used as field name.

If a field has the same name as a keyword, the parser will generate an error if the setting `error` is true (default)
of a warning and will prefix the name with an underscore to avoid a syntax error in the generated file.

```toml
# Optional: Specify reserved keyword handling
# This example correspond to the default value
[keywords]
sv = true
vhdl = false
error = true
```

## Target-Specific Configuration

Each generator can have its own options, and can override also default setting such as `gen_inc` or `local`.

### HTML Generation

```toml
[html]
# Optional: Path to custom CSS file
css = "./custom.css"

# Optional: Split output into multiple files (one file per RIF)
split = true
```

### AsciiDoctor Generation

```toml
[adoc]
# Optional: Split output into multiple files (one file per RIF)
split = true

# Optional: Override global gen_inc for this target
gen_inc = ["module1", "module2"]

# Optional: Override global local for this target
local = ["*"]
```

### C Header Generation

```toml
[c]
# Optional: Name of base address offset constant (default: PERIPH_BASE_ADDR)
base_offset = "CUSTOM_BASE_ADDR"

# Optional: Override global gen_inc for this target
gen_inc = ["*"]

# Optional: Override global local for this target
local = ["module1"]
```

### RTL Generation (SystemVerilog/VHDL)

```toml
[rtl]
# Optional: Number of pipeline levels for register access (default: 1)
nb_pipe = 2

# Optional: Override global gen_inc for this target
gen_inc = ["*"]

# Optional: Override global local for this target
local = ["core_module"]
```

### Register Abstraction Layer (RAL)

```toml
[ral]
# Optional: Name of register block class (default: uvm_reg_block)
class = "my_reg_block"

# Optional: Macro name for RAL instantiation (default: ral_create_reg_block)
macro_name = "MY_RAL_MACRO"

# Optional: Override global gen_inc for this target
gen_inc = ["*"]

# Optional: Override global local for this target
local = ["verification_module"]
```

### Python Generation

```toml
[py]
# Optional: Python class naming convention
class = "MyPeripheral"

# Optional: Target Python version
# Valid values: "3.8", "3.9", "3.10", "3.11", "3.12"
version = "3.10"

# Optional: Override global gen_inc for this target
gen_inc = ["*"]

# Optional: Override global local for this target
local = ["driver_module"]
```

### SVD (System View Description)

```toml
[svd]
# Optional: Vendor name for SVD file
vendor = "MyCompany"

# Optional: Version string for SVD file
version = "1.0.0"
```

### IP-XACT Generation

```toml
[ipxact]
# Optional: Vendor identifier
vendor = "mycompany.com"

# Optional: Library name
library = "peripherals"

# Optional: Version string
version = "2.1.0"
```

### MIF (Framemaker) Generation

```toml
[mif]
# Optional: Split output into multiple files (one file per RIF)
split = true

# Style definitions for different elements
anchor = "Anchor"
h2 = "Heading2"
h3 = "Heading3"
table_title = "TableTitle"
table_heading = "TableHeading"
table_cell = "TableCell"

# Table styles by type
table_kind_rifmux = "RifMuxTable"
table_kind_mapping = "MappingTable"
table_kind_reg = "RegisterTable"

# Column width specifications
width_3col = [2.0, 3.0, 4.0]    # For 3-column tables (RifMux, Pages)
width_4col = [1.5, 2.0, 1.0, 3.0]  # For 4-column tables (registers)
width_5col = [1.0, 1.5, 1.0, 1.0, 3.0]  # For 5-column tables (fields)

# Optional: Override global gen_inc for this target
gen_inc = ["*"]

# Optional: Override global local for this target
local = ["doc_module"]
```

## Complete Example

```toml
# Basic configuration
filename = "my_soc.rif"
include = ["./common", "../ip_library"]
gen_inc = ["*"]
local = ["core_ip"]
targets = ["sv", "c", "html", "py", "ral", "svd"]
public = false
suffix_rtl_only = true

# Parameters
[parameters]
DATA_WIDTH = 32
NUM_INTERRUPTS = 16
BASE_ADDR = 0x40000000

# Output paths
[outputs]
rtl = "./generated/rtl"
c = "./generated/sw/include"
html = "./docs/html"
py = "./python/drivers"
sim = "./verification/ral"
svd = "./sw/svd"

# Suffixes
[suffixes.spi]
name = "full"
pkg = true
alt_pos = true

# Target-specific settings
[html]
css = "./docs/custom.css"
split = true

[c]
base_offset = "SOC_BASE_ADDR"

[py]
version = "3.10"
class = "SocPeripheral"

[ral]
class = "soc_reg_block"
macro_name = "SOC_RAL_CREATE"

[svd]
vendor = "MyCompany"
version = "1.2.0"

[rtl]
nb_pipe = 1
```
