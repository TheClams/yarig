# RIF FAQ / HOWTO

This document provides answers to common questions and syntax examples for implementing registers and fields in RIF files.

---

## Table of Contents

1. [Read-Only Fields](#read-only-fields)
2. [Constant Fields](#constant-fields)
3. [Fields Larger Than Register Width](#fields-larger-than-register-width)
4. [Reusing Register Definitions](#reusing-register-definitions)
5. [Array Fields Larger Than Register Width](#array-fields-larger-than-register-width)
6. [Write-Only Fields](#write-only-fields)
7. [Hardware-Accessible Fields](#hardware-accessible-fields)
8. [Fields with Reset Values](#fields-with-reset-values)
9. [Interrupt Registers](#interrupt-registers)
10. [Enumeration Fields](#enumeration-fields)
11. [Counter Fields](#counter-fields)
12. [Conditional/Optional Registers](#conditionaloptional-registers)
13. [Signed Fields](#signed-fields)
14. [Fixed-Point Fields](#fixed-point-fields)
15. [Password Fields](#password-fields)
16. [Pulse Fields](#pulse-fields)
17. [Fields with Write Limits](#fields-with-write-limits)

---

## Read-Only Fields

**Q: What is the syntax for a read-only field?**

A: Use the `ro` keyword after the bit range. If no reset value is provided, the field defaults to read-only.

```yaml
- status: "Status register"
  - state = 0 3:0 ro "Current state"
  - ready 4:4 ro "Ready signal"
  - error 5:5 "Error flag"  # Also read-only (no reset value = ro by default)
```

---

## Constant Fields

**Q: What is the syntax for a constant field readable from both hardware and software?**

A: Define a read-only field with `hw na` (hardware not accessible) or `hw r` (hardware read-only). For a true constant that neither hardware nor software can modify, use `ro` with `hw na`:

```yaml
- version: "Version register"
  - version = 0x12 15:8 ro "Block version"
    hw na  # Hardware cannot access this field
  - chip_id = 0xABCD 31:16 ro "Chip identifier"
    hw r   # Hardware can read but not modify
```

---

## Fields Larger Than Register Width

**Q: How to handle read-only fields which have a size larger than a register?**

A: Use the `partial` property to split a field across multiple registers. The field name must be identical in each register, and you specify the LSB position of the larger field:

```yaml
- data_low: "Lower 32 bits of 64-bit data (must be read first)"
  - data = 0 31:0 ro "Data bits [31:0]"
    partial 0
- data_high: "Upper 32 bits of 64-bit data"
  - data = 0 31:0 ro "Data bits [63:32]"
    partial 32
    we rif_data_low.p_read
```

This creates a single 64-bit `data` field split across two 32-bit registers. When the register `data_low` is read, the MSB are stored inside a local register guarantying coherence between LSB and MSB.

---

## Reusing Register Definitions

**Q: What is the syntax to re-use a register definition but with different description and reset values for some fields?**

A: Use the `- name = type` in the instance level or `- name[4]` to have an array of registers

```yaml

registers:
  - ctrl: "Control register"
    - enable = 0 0:0 rw "Enable bit"
    - mode = 0 3:1 rw "Mode selection"

instances:
  - ctrl_block0 = ctrl @ 0x00
    description: "Control register for Block0"
    enable.description "Enable Block0"
    mode.reset = 5    # Override mode reset value
  - ctrl_block1 = ctrl @ 0x01
    description: "Control register for Block1"
    enable.reset = 0  # Override reset value
    enable.description "Custom enable description"
    mode.reset = 5    # Override mode reset value
  - ctrl[4] @ 0x02
    [0].description "Control or block[0]"
    [3].mode = 4
```

**Q: How to import a register definition from another RIF ?**

A: Use the `include` syntax to include a register, then override properties at the instance level:

```yaml
registers:
  - include base_rif.ctrl

instances:
  - ctrl @ 0x00
    description: "Custom control register for this instance"
    enable.reset = 1  # Override reset value
    enable.description "Custom enable description"
    mode.reset = 5    # Override mode reset value
```

You can also override at the register definition level using field properties:

```yaml
registers:
  - include base_rif.ctrl
    enable.description "Different description"
    enable.reset = 1
```

---

## Array Fields Larger Than Register Width

**Q: How to handle array of fields larger than the register width?**

A: Use `arrayPartial` property to split an array across multiple registers:

```yaml
- reg1: "First register with coefficient array"
  - coeff[3] = {0x01, 0x12, 0x23} 14:8 "Coefficient value $i"
    arrayPosIncr 8
- reg2: "Second register continuing coefficient array"
  - coeff[2] = {0x34, 0x45} 6:0 "Coefficient value $i"
    arrayPosIncr 8
    arrayPartial 3  # Continue from index 3
```

This creates a `coeff` array with 5 elements (indices 0-4) with 7 bits width, split across two registers.

The `arrayPosIncr` allows to position each fields on a byte boundary:
  - `coeff[0]` is in `reg1[14: 8]`
  - `coeff[1]` is in `reg1[22:16]`
  - `coeff[2]` is in `reg1[30:24]`
  - `coeff[3]` is in `reg2[ 6:0]`
  - `coeff[4]` is in `reg2[14:8]`

Alternatively, the register can also be defined as an array:

```yaml
- coeff[4]: "Complex Coefficients $i"
  - real[1] = {-100,-200,-300,-400} 15:0 "Coefficient real part $i"
  - imag[1] = {+100,+200,+300,+400} 31:16 "Coefficient imaginary $i"
```
This creates a group of 4 register containing two arrays of 4 elements each with 16 bits signed.

---

## Write-Only Fields

**Q: How to define a write-only field?**

A: Use the `wo` keyword:

```yaml
- command: "Command register"
  - cmd = 0 7:0 wo "Command value"
  - trigger = 0 8:8 wo "Trigger command"
```

Read of a register containing only `wo` fields will generate a bus access error. If the `wo` fields are mixed with readable fields, their read-value is simply forced to 0.

---

## Hardware-Accessible Fields

**Q: How to define fields that hardware can read/write?**

A: Use the `hw` property with `r`, `w`, `rw`, or `na`:

```yaml
- status: "Status register"
  - state = 0 3:0 ro "Current state"
    hw rw  # Hardware can read and write
  - config = 0 7:4 rw "Configuration"
    hw r    # Hardware can only read
  - control = 0 11:8 rw "Control bits"
    hw w    # Hardware can only write
```

Note that the field `control` is automatically updated to have a write-enable signal to allow the simultaneous write access from hardware and software.

---

## Fields with Reset Values

**Q: How to specify reset values in different formats?**

A: Reset values support decimal, hexadecimal, parameters, and enum values:

```yaml
- config: "Configuration register"
  - mode = 0 2:0 rw "Mode selection"           # Decimal
  - addr = 0x1000 15:4 rw "Base address"       # Hexadecimal
  - count = $DEFAULT_COUNT 31:16 rw "Counter" # Parameter
  - enable = 1 0:0 rw "Enable bit"            # Decimal (1)
  - signed_val = -5 7:0 rw "Signed value"     # Negative (auto-signed)
```

---

## Interrupt Registers

**Q: How to define interrupt registers with enable, mask, and pending?**

A: Use the `interrupt` keyword on the register with optional `en`, `mask`, and `pending`:

```yaml
- interrupt: "Interrupt status register"
  interrupt rising en=0x13 mask=0x37 pending w1clr
  enable.description "Enable interrupt sources"
  mask.description "Mask interrupt sources"
  pending.description "Pending interrupt status"
  - gen = 0 7:0 rw "Generic Events"
  - busy = 0 8:8 "Busy interrupt"
  - ready = 0 9:9 "Ready interrupt"
```

This automatically creates:
- `interrupt_en` register for enabling interrupts
- `interrupt_mask` register for masking interrupts  
- `interrupt_pending` register (read-only) showing pending interrupts

---

## Enumeration Fields

**Q: How to define fields with enumerated values?**

A: Use the `enum` property:

```yaml
- decoder: "Viterbi decoder control register"
  - coding_rate = 0 4:2 rw "Coding rate selection"
    enum: e_cr
      - CR_1_2 = 0 "Code rate 1/2"
      - CR_2_3 = 1 "Code rate 2/3"
      - CR_3_4 = 2 "Code rate 3/4"
```

To generate a typedef enum, use `enum: type` or `enum: <type_name>`:

```yaml
  - coding_rate = 0 4:2 rw "Coding rate selection"
    enum: type  # Generates typedef enum e_decoder_coding_rate
      - CR_1_2 = 0 "Code rate 1/2"
      - CR_2_3 = 1 "Code rate 2/3"
```

---

## Counter Fields

**Q: How to define counter fields?**

A: Use the `counter` property with `up`, `down`, or `updown`:

```yaml
- stats: "Statistics register"
  - tx_count = 0 31:0 ro "Transmit counter"
    counter up incrVal clr
    hw rw  # Allow hardware to read counter value
  - rx_count = 0 31:0 ro "Receive counter"
    counter updown incrVal decrVal sat  # Saturating up/down counter
```

The `incrVal` and `decrVal` create input signals for increment/decrement values. Use `clr` for a clear signal and `sat` for saturation.

---

## Conditional/Optional Registers

**Q: How to make registers conditional based on parameters?**

A: Use the `optional` property with a condition expression:

```yaml
parameters:
  - HAS_FEATURE_X = 1

registers:
  - feature_x: "Feature X register"
    optional: $HAS_FEATURE_X == 1
    - enable = 0 0:0 rw "Enable feature X"
```

Fields can also be conditional:

```yaml
  - mode = 0 3:0 rw "Mode selection"
    optional: $WIDTH < 16 * $FIELD_EN
```

At the hardware level, the field still exists but it is constant (reset value)

---

## Signed Fields

**Q: How to define signed fields?**

A: Fields are automatically signed if the reset value starts with `+` or `-`, or use the `signed` property:

```yaml
- data: "Data register"
  - value = -5 15:0 rw "Signed value"  # Auto-signed from negative reset
  - offset = 0 31:16 rw "Offset value"
    signed  # Explicitly signed
```

---

## Fixed-Point Fields

**Q: How to define fixed-point fields with fractional bits?**

A: Use the `nb_frac` property:

```yaml
- config: "Configuration register"
  - gain = 1.5 15:0 rw "Gain value"
    nb_frac 4  # 4 fractional bits (s12.4 format)
```

The reset value `1.5` with 4 fractional bits becomes `24` (1.5 × 2^4 = 24). In descriptions, use `$f` to display the format (e.g., `s12.4`).

---

## Password Fields

**Q: How to define password-protected fields?**

A: Use the `password` property:

```yaml
- unlock: "Unlock register"
  - password = 0 31:0 wo "Password field"
    password once=0xDEADBEEF hold=0xCAFEBABE protect
```

This creates a `<regname>_<fieldname>_locked` signal that:
- Starts locked (1)
- Unlocks when password matches `once` value (stays unlocked)
- Unlocks when password matches `hold` value (stays unlocked until wrong password)
- `protect` locks permanently if wrong password (except 0) is written

---

## Pulse Fields

**Q: How to define pulse fields that stay high for one cycle?**

A: Use the `pulse` property:

```yaml
- ctrl: "Control register"
  - start = 0 0:0 rw "Start pulse"
    pulse  # Registered pulse (one cycle delay)
  - reset = 0 1:1 rw "Reset pulse"
    pulse comb  # Combinatorial pulse (immediate)
```

---

## Fields with Write Limits

**Q: How to restrict valid write values for a field?**

A: Use the `limit` property:

```yaml
- config: "Configuration register"
  - mode = 0 3:0 rw "Mode selection"
    limit [0:7]  # Only values 0-7 are valid
  - rate = 0 7:4 rw "Rate selection"
    limit {1,2,4,8}  # Only these specific values
  - enum_field = 0 11:8 rw "Enum field"
    enum: type
      - VAL_A = 0 "Value A"
      - VAL_B = 1 "Value B"
      - VAL_C = 2 "Value C"
    limit enum  # Automatically limit to enum values
```

Invalid writes will not update the register and raise an access error.

---

## Additional Tips

### Field Position Syntax

You can use different syntaxes for field positions:

```yaml
- reg: "Register example"
  - field1 = 0 15:8 rw "MSB:LSB syntax"
  - field2 = 0 8+:8 rw "LSB+:WIDTH syntax"
  - field3 = 0 8 rw "Auto-positioned (next available bit)"
```

### Hardware Set/Clear/Toggle

```yaml
- status: "Status register"
  - flag = 0 0:0 rw "Flag bit"
    hwset        # Auto-generated set signal
    hwclr        # Auto-generated clear signal
    hwtgl        # Auto-generated toggle signal
```

Or with custom signals:

```yaml
  - flag = 0 0:0 rw "Flag bit"
    hwset self.enable  # Use field from same register
    hwclr .internal_clr  # Use internal signal
```

### Clock and Reset Control

```yaml
- special_reg: "Special register"
  clock hw_clk  # Use hardware clock
  hwReset hw_rst  # Use hardware reset
  clkEn my_clk_en  # Clock enable signal
  - data = 0 31:0 rw "Data"
```

### Field-Level Clock Enable

```yaml
  - data = 0 31:0 rw "Data"
    clkEn data_clk_en  # Field-specific clock enable
```

---

## See Also

- [RIF Syntax Documentation](syntax.md) - Complete syntax reference
- [Configuration Documentation](config.md) - Generator configuration options
- [Targets Documentation](targets.md) - Output format details

