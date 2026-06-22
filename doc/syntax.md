# RIF syntax

The syntax for the RIF is loosely inspired by YAML: a human readable text file with structure based on indentation.
It supports comments starting with `//` or `#`.

It is composed of different levels:
 - [Top](#Top) : Top level structure, only one per file, contains settings, parameters and pages
 - [Page](#Page): group of register and register instance, with a base address
 - [Registers](#Register): Register declaration, regrouping field
 - [Field](#Field) : N bits inside a register
 - [RegisterInstance](#Registerinstance): Instance name of a register with a given address

The convention notation when describing syntax is the following:
 - name inside angle bracket (e.g. `<value>`) represents a mandatory value (integer/string/expression depending on the context)
 - name inside straight bracket represents optional values or keyword  (e.g. `[<value>]` or `[kw]`)
 - List of label separated by pipe (e.g. `ro|rw|rclr`) represents a list of valid keywords expected. This can be combined with straight bracket for list of optional keywords.

The casing convention is snake_case in the RIF definition: generators have option to individually change the casing of the generated files.

## Example
Here is a simple RIF definition:
```yaml
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

---


## Top
The component regroups some top level information about the RIF, definition of signals and parameters.
Syntax is `rif: <rifName>` where rifName is the name which be used for the RTL module name as well as the base filename of all outputs.

The possible properties, indented by one level compare to the RIF declaration, are:

 - `addrWidth : <addrWidth>` : Number of bits of the address bus (byte aligned)
 - `dataWidth : <dataWidth>` : Number of bits of the data bus
 - `interface : <ifname>` : Define the type of interface used for the RIF. Possible value are default, apb, uaux.
  By default uses a memory like interface (with a done signal asserted when access is complete).
 - `description : <blabla>` : Provides some high level information.  Mainly for documentation (e.g. HTML output).
   Quotation mark are optional and removed for the first line.
   Description can be on multiple lines as long as it is indented by at least one level compare to the keyword description. For example:
```yaml
  description:
    First line of description
    Second line of description
```
 - `swClock : <clock_name>` : Name the software clock signal _clock_name_ (clk by default)
 - `hwClock : <clock_name0> <clock_name1> ...` : Declare hardware clocks. The first one will be the default one.
 - `swReset|hwReset : <rst_name> [activeLow|activeHigh] [async|sync]`: Specify reset signal for software/hardware.
 - `swClkEn|hwClkEn : <clk_en_name> `: Specify default clock enable signal for software/hardware clocks. This can be overriden on a register basis
 - `swClear : <clear_name>` : Declare a global clear _clear_name_ active high to clear all software register (no clear by default)
 - `hwClear : <clear_name0> <clear_name1> ...` : Declare global hardware clears _clear_name_ active high to clear all hardware register (no clear by default). There should be as many signal declared as there are hwClock: same name can be repeated and the minus character `-` can be used indicate that there is no clear for the corresponding hwClock.
 - `suffixPkg : <boolean>` : Apply suffix to the generated package file as well (default: false)
 - `parameters : `: Start a parameter list. See [paragraph Parameters](#Parameters).
 - `generics : `: Start a generic list. See [paragraph Generics](#Generics).
 - `- pageName : [description]` : Start a page named pageName. See [below](#Page). The name is used only inside the documentation.

The only mandatory property is at least one page.
By default the address is on 16b, the data on 32, the clock is named `clk`,
the reset is `rst_n` (asynchronous, active low) and used for both hardware and software logic.


## Parameters

A parameter is pair name/value declared with `- <NAME> = <value_or_expr>`.

Parameters can be used as reset value, field width, array size (both register and field): simply prefix the name of the parameter by `$`

Declaration of parameter can use arithmetic operation and reference other parameters: `- PARAM1 = 2*$PARAM2-1 + ceil(log2($WIDTH))`.
It supports all basic arithmetic operation and a few functions like pow, log2, log10, ceil and floor.
Note that pow, log2 and log10 output is a floating point number and should typically be associated with ceil/floor operation when the parameter value is used for field width or array size.

Parameters value can be overridden by a command-line argument `--parameters <name>=<value>`

## Generics

A generic is defined as `- <name> = [<min>:]<default>:<max> "description"`.

This generate an RTL with input parameter.
The generic value can be used for register instances array size, control of optional register or optional component in a RifMux.
If no minimum value is provided, it is set to 1.


## Page
The page contains the definition of n [registers](#Register) , their [register instances](#Registerinstance) and a few page properties.

The available options, indented by one level compare to the page, are:

 - `baseAddress <offset>` : Address offset of all register inside the page. Format can be decimal (64) or hexadecimal (0x20)
 - `addrWidth <nb_bit>` : Define the page range in Number of bits. Format can be decimal (64) or hexadecimal (0x20). This is only required for external pages
 - `description` : Page description. Mainly for documentation (e.g. HTML output).
 - `clkEn : <clock_enable_name>` : Define a clock enable signal for all register in the page
 - `external` : Indicates that the page logic is external. The only logic provided will be the address decoding.
 - `optional : <condition>` : Indicate that the register of a page are instantiated only if the _condition_ is true.
  The condition should be a valid arithemtic expression where parameters can be used: this supports standard math operation (+,-,\*,\%,<<,>>) or comparison operator (==,!=,>,<,...).
 - `registers:` : Start the register declaration entry. See [below](#Register) for detail.
 - `instances:` : Start the register instances entry. See [below](#Registerinstance) for detail.
 - `include <rifName>.<pageName>` : Include a page from another RIF, both register definition and instance. This cannot be mixed with manual registers/instances entry !


## Register
The register is a group of fields with some basic properties.

Its declaration is indented by one level compare to the registers keyword and follow the syntax:
`- <reg_name>: [(<regGroup>)] ["Register short description"]`

The optional register group is specific to the RTL view and allows to regroup multiple registers under the same structure.
regGroup can be in the form of rifname::groupname to reference a structure from another RIF.


The register properties, indented by one level compare to the register declaration, are:

 - `clock <clock_name>` : Indicate which clock (amongst the software and all hardware clocks) should be used for this register.
    By default the first hardware clock is used for all register with write access from hardware, and the software clock used for all others.
 - `hwReset <reset_name>` : Indicate which reset (amongst all hardware reset) should be used for this register.
    By default the swReset is used for every register controled by software and the hwReset for every register controlled by hardware.
    If the clock is changed manually the reset should also be set manually
 - `clkEn <clock_enable_name>` : Define a clock enable signal for all fields in the register. If a global clock enable was defined, the value False can be used to have a register with no clock enable.
 - `description: <long description>` : More detailled register description. Mainly for documentation (e.g. HTML output).
    Description can be on multiple lines as long as it is indented by at least one level compare to the keyword description
    Description can contains latex equation enclosed between backtick: e.g. ``eq=`\\frac{e^3}{c_1 + c_2}` ``
 - `description.hidden: <extra description` : Extra description displayed only when documentation is generated without the --doc_hide flag
 - `mask|enable.description <long description>` : For interrupt register, set the description of the associated mask and enable register.
 - `external` : The register logic is handled outside of the RIF which provides additional signal related to address decoding.
 - `externalDone` : The register logic for the done access is handled outside of the RIF, allowing to add wait state.
 - `wrPulse|rdPulse|accPulse [comb|reg] [<clock_name>]` : A pulse is generated when the register is, respectively written, read or both.
    The pulse can be combinatorial (i.e. the pulse happens at the same time as the register access) or registered (i.e. one clock latency).
    By default, the read pulse is combinatorial while the write/access are registered.
    When registered, a clock can be provided to indicate a different clock than the register one to generate the pulse.
 - `interrupt high|low|rising|falling|edge [en[=<val_enable>]] [mask[=<val_mask>]] [pending] [rclr|wclr|w0clr|w1clr|hwclr]` : Indicates that all fields in the register are interrupts, active either on a level (high or low) or an edge (rising, falling or both).
    The optional enable property will auto declare a register to enable the interrupt with same name as the interrupt register plus \_en with a reset value _valEnable_ (for the whole register). Every hardware interrupt goes through an and of the enable register before updating the status register.
    The optional mask property will auto declare a register to mask the interrupt with same name as the interrupt register plus \_mask with a reset value _valMask_ (for the whole register). The interrupt request signal is the OR of an AND between the status register and the mask register. So, when the value for a field is 0, the corresponding interrupt status is asserted but this does not trigger the interrupt request.
    The optional pending property (only allowed if a mask is used) will auto declare a read-only register containing the AND of the status and the mask (which was used to generate the external interrupt request signal).
    Default to level high, with clear on read with no mask, enable or pending if no information is provided.
 - `alt <name> [en[=<val_enable>]] [mask[=<val_mask>]]  [pending]  ["<description>"]`: only valid for interrupt register.
    Allows to define an alternative interrupt register with a different enable/mask/pending settings and a secondary interrupt request output.
    This is useful when needing two configurable interrupt lines with different priority using the same set of interrupt event
 - `clear` : Synchronous clear signal for the register, setting all fields to their reset values
 - `hidden` : Hide the register in any documentation (HTML, MIF, C Header) when the visibility is set to public (generator setting)
 - `disabled` : Force the register to its reset value. Used typically when overloading included register.
 - `reserved` : Rename register in any documentation to `rsvd` when the visibility is set to public (generator setting). Also remove description.
 - `optional : <condition>` : Indicate that the register is defined only if the _condition_ is true or different from 0.
    The condition should be a valid arithmetic expression where parameters can be used (e.g. `$REG1_EN==1`): this supports standard math operation (+,-,\*,\%,<<,>>) or comparison operator (==,!=,>,<,...).
 - `optional_acc: [na|ro|rw]`: Control the software access (ro) of register which are due to parameters value. Default is `na` (any access will generate a bus access error).
 - `- <fieldName> ...` : Define a fields named _fieldName_ inside the register. See [below](#Field) for detail.

It is also possible to include the definition from another rif file using the following syntax:

 - `- include <rifName>.*` : Include all register definition
 - `- include <rifName>.<pageName>.*` : Include all register defined in a page
 - `- include <rifName>.<regName>` : Include only one register


## Field
The field is group of bits inside a registers.

Its declaration is indented by one level compare to the register declaration and follow the syntax:
`- <field_name> [= <reset_value>]  <msb>:<lsb> [ro|rw|rclr|w1clr|w0clr|w1set|pulse|pulsecomb] ["Field short description"]`

The `reset_value` supports multiple format:
  - decimal value: if it starts by + or -, the field will be automatically flagged as `signed`
  - hexadecimal value using C or SystemVerilog notation (`0xABCD` or `11'h4CD`)
  - parameter value using `$<PARAM_NAME>`
  - enum value if the field has an enum property defined
  - floating value if the field has a number of fractional bit defined (e.g. for a field with 3 fractional bit a reset value of `4.125` will be translated as a reset value of 33).

The field position can also use the systemVerilog range syntax `<lsb>+:<width>` instead of `<msb>:<lsb>`
or even just `<width>` in which case the field is position on the next available bit (starting from 0).
The `lsb`, `msb` and `width` must be integer or a parameter (but not an arithmetic expression).
The alternative syntax is particularly useful when the width is a parameter.

Following the field position is the field software access kind: it is optional and its default value
is `ro` if no reset value is provided and `rw` if a reset value is provided.

The default hardware access kind depends on the software access:
 - If a register can be read/write by the software then the hardware access is in read-only
 - If the register is in read-only for the software then the hardware access is write-only

If both software and hardware can modify a register value, then the hardware access needs
some mechanism to indicate when its value should be used to modify the register value (via `we|wel|hwset|hwclr|hwtgl`)

The optional properties of a field, indented by one level compare to the field declaration, are:

 - `description: <long description>` : More detailed register description. Mainly for documentation (e.g. HTML output). Quotation mark are removed on the first line. Description can be on multiple lines as long as it is indented by at least one level compare to the keyword description
 - `description.hidden: <extra description` : Extra description displayed only when documentation is generated without the --doc_hide flag
 - `mask|enable|pending.description <long description>` : For interrupt register, set the description of the associated mask/enable/pending register field. Again, description can be done on multiple lines.
 - `hw [r|w|rw|na]` : Specify a type of access from the hardware
  - `r` : read-only (Default)
  - `w` : write-only
  - `rw` : read/write
  - `na` : Not accessible from HW
 - `clock <clock_name>` : Indicate which clock (amongst the software and all hardware clocks) should be used for this field. Default to use the register clock.
 - `clkEn <clock_enable_name>` : Define a clock enable signal for this field
 - `hwset [<set_signal>] [<data_signal>]` : Hardware high set the field to 1.
  The _set_signal_ can be in the form `self.field_name` in which case a field is added to the hardware structure of the register (if it does not exists).
  If _set_signal_ is in the form `reg_name.signal_name`, then it references a field in another register structure.
  If _set_signal_ is simply of the form `signal_name`, then a port with this name is added.
  If _set_signal_ starts with a dot like `.signal_name`, then it reference an internal signal of the module (for example the address decode signals).
  If no _set_signal_ is provided, a field with the name _reg_name_hwset_ is automatically added.
  When used on multi-bit field a _data_signal_ must be provided: each bit high will set the corresponding bit high in the field when the _set_signal_ goes high.
 - `hwclr [<clr_signal>] [<data_signal>]` : Hardware high set the field to 0.
  If no _clr_signal_ is provided, a field with the name _reg_name_hwclr_ is automatically added.
  When used on multi-bit field if no _data_signal_ is provided the whole field is set to 0. Otherwise only bits high in _data_signal_ are reset to 0 when _set_signal_ goes high.
 - `hwtgl [<tglSignal>] [<data_signal>]` : Hardware high toggle the field value.
  If no _tglSignal_ is provided, a field with the name _reg_name_hwtgl_ is automatically added.
  When used on multi-bit field if no _data_signal_ is provided the whole field is inverted. Otherwise only bits high in _data_signal_ are toggled _set_signal_ goes high.
 - `lock [<lock_signal>]` : Signal to prevent a register to be written. _lock_signal_ follow the same rule as _set_signal_ just above.
 - `pulse [comb]` : The field stays high only one cycle after being set. If pulse is followed by `comb`, then the pulse is generated on the write signal without extra flop in the block.
 - `toggle` : When a 1 is written by software on this field, the field inverts its value.
 - `signed` : Indicates that the value stored is a signed value.
 - `we [<weSignal>]` : The copy from hardware to the RIF field value is done only when a write enable signal is high. Valid only for field with write access from hardware.
 - `wel [<welSignal>]` : Same as 'we' but write enable signal is active low.
 - `clear [<clearSignal>]` : Synchronous clear, setting the field to its reset value. _clr_signal_ follow the same rule as the hwset/hwclr/hwtgl.
  This property is slightly different from the hwclr: first the field is set to its default reset value instead of just 0
  and second the clear signal bypass the clock enable
 - `counter up|down|updown [incrVal[=<width>]] [decrVal[=<width>]] [clr] [sat]`: The field is a counter up and/or down, with optional input signal for the increment/decrement value and the clear.
  By default the increment/decrement value signals have the same width as the counter but this can be changed by specifying a number after the incr/decrVal keyword.
  Note: the counter value is not accessible by default from hardware, but this this can be changed with `hw rw` property which must be placed after the counter declaration.
 - `partial <lsb_pos>` : Indicates that this field is larger than the register and that the LSB correspond to the bit _lsb_pos_ of a larger field. The field name must be identical in each register definition.
 - `interrupt high|low|rising|falling|edge [en=<val_enable>] [mask=<val_mask>] [rclr|wclr|w0clr|w1clr|hwclr]` : override default interrupt settings.
 - `limit ([<min>:<max>]|{<v0>,<v1>,<...>}|enum|external) [<bypass_signal>]` : Limit valid write value for a field.
    The limit can be a range in the form of `[min:max]` (min & max included, one can be omitted), a set of value with `{v0,v1,v2}`, an external logic (keyword `external`)
    or, when the field is an enum using the keyword `enum` will automatically limit the value to the enumerated values.
    When writing an invalid value, the register will not be updated and an access error will be raised on the control bus.
    If a bypass signal is provided, when the signal is high the limit is ignored.
 - `nb_frac [value]` : Number of fractional bits for fixed-point representation. This can be used in description with `$f` to display the format of the field:
    for example `s7.4` or `u8.10` for respectively a signed 7 bits wide field  with 4 fractional bits (range [-4:3.9375] with a precision of 1/16)
    or an unsigned 8 bits wide field with 12 fractional bits (range [0:0.249] with a precision of 1/1024).
 - `password [once=<val>] [hold=<val>] [protect]` : The field is used to generate an internal signal named `<regname>_<fieldname>_locked` which is initialized to 1
  and reset to 0 if the written value match one of the password value. If the password corresponds to the once value the field will stay low until the next write.
  If it corresponds to the hold value, it stays low until a value different from the a valid password is written.
  The optional `protect` will lock the password until the next reset if a wrong value (except 0) is written.
  This field has automatically the property `hidden` (i.e. won't appear in public documentation).
  The read value correspond to the state of the password: 1 for locked, 0 for unlocked once, 2 for unlocked hold and 3 for stuck (i.e. need reset).
 - `hidden` : allows to hide the field in any documentation (HTML, MIF, C Header) when the visibility is set to public (generator setting)
 - `disable` : force the field to the reset value. Used typically when overloading included register.
 - `reserved` : Renamed field in any documentation to `rsvdxx` (where xx is the LSB) when the visibility is set to public (generator setting). Also remove description.
 - `optional : <condition>` : Indicate that the field is defined only if the _condition_ is true.
  The condition should be a valid arithmetic expression where parameters can be used. For example: `$WIDTH<16 * $FIELD_EN`

The value described as signals (_set_signal_, _clr_signal_, _tglSignal_) can not only be signal name but also simple logical expression, using systemVerilog syntax.
For example `hwset (self.field_en && !reg1.freeze)`.

Some fields can be declared as `enum`, with description of each possible value.
This starts by adding the property enum with `enum : [type|<type_name>|<pkg::name>]`:
 - If no type is provided the enum values are only part of the documentation.
 - If the keyword type is provided a typedef enum will be declared with the type name `e_regname_fieldname`.
 - If a simple type name is used, this override the default value of type.
 - If the type name is preceded by a scope package (`my_pkg::my_type`), then the type is assumed to be defined in another package.
The enum property must then be followed by a list of all possible value, indented one level compare to the enum property, following the syntax `- <name> = <value> [(<reprValue>)] "description"`.
The optional _reprValue_ can be an integer or a float: it allows to provide the real value behind an enum for example when the enum is a list of data-rate
For example:
```yaml
  - decoder: "Viterbi decoder control register"
    - coding_rate = 0    4:2   rw "Coding rate selection"
      enum: e_cr
        - CR_1_2    = 0 "Code rate 1/2"
        - CR_2_3    = 1 "Code rate 2/3"
        - CR_3_4    = 2 "Code rate 3/4"
```

A field can also be declared as array by simply giving the array size with the fieldname: `- <fieldName>[<arraySize>] ...`:
  - The reset value can be given either as a single value when all element have the same value or as an array of value enclosed in curly bracket (e.g. `- arr[5] = {1,2,3,4,5}`).
  - The position of the field corresponds to the position of the field at index 0.
  - The others are auto-instantiated, with a position that increment by the size of the fields by default. To change the increment, add the property `arrayPosIncr <value>`.
  - It is also possible to have an array that extends on more than one register: this works like the partial field, with the property `arrayPartial <offset>`.
    The registers must belong to the same register group.
    Another possibility is to declared the register itself as an array: the total field array size will be given by
    the register array size multiply by the field array size or the length of the reset value array if it is bigger than the field array itself.
    In this case all fields of the register must be arrays.
  - In the description, the field index can be referenced by `$i` or even in a formula such as `${2*i+1}`.

First example of field array split on two distinct register. This create a field array `coeff` with a total size of 5 elements of 7 bits.
The first three elements are inside `reg1` starting at bits 8, 16 and 24 due to the use `arrayPosIncr` while the last two element are inside `reg2` at position 0 and 8.
The description of each `coeff` register will be "Coefficient value 0", "Coefficient value 1", ...
```yaml
  - reg1 : (ctrl) "Control register 1"
    - enable = 0 0:0 "Enable block"
    - status   0 7:4 ro "Block status"
    - coeff[3] = {0x01,0x12,0x23} 14:8 "Coefficient value $i"
      arrayPosIncr 8
  - reg2 : (ctrl) "Control register 2"
    - coeff[2] = {0x34,0x45} 6:0  "Coefficient value $i"
      arrayPosIncr 8
      arrayPartial 3
```

This second example shows the use of a register array definition. The total size of fields key and val is 5.
```yaml
  - reg_kv[5] : (ctrl) "Key value pairs $i"
    - key[1] = {0,1,2,3,4} 7:0 "Key $i"
    - val[1] = {0x101,0x202,0x303,0x404,0x505} 27:8 "Value $i"
```


## RegisterInstance
If all the register declared have to be instantiated just once, in order, the simplest way is to simply use `instances: auto` or `instances: auto-legacy`.

The `auto-legacy` mode uses legacy order for interrupts (mask before enable), while `auto` uses the standard order (enable before mask).

If the auto keyword is not used, then all instance of register are added with the following syntax,
indented by one level compare to the instances keyword:
`- <reg_name> [= <regType>] [(<groupName>)] [@ <regAddr>]`

By default the register type is the same as the register name, and the address is auto incremented compare to the previous register.

The address is always aligned on byte, no matter what the address width.

The _groupName_ is only necessary when there is multiple instances of register belonging to a register group.

It is possible to override some properties at the instance level. Here the syntax to use, indented by one level
 - `description : <bla bla>` : register description, can be multi-line as long as it is indented by another level. Quotation mark on first line are removed.
 - `hw r|w|rw|na` : change the hardware access of a register
 - `<field_name>.description <bla bla>` : change a field description
 - `<field_name>.reset = <rst_val>` : change a field reset value
 - `<field_name>.disable [= <disable_val>]` : disable a field (i.e. value is fixed). If no value is provided, it default to the reset value.
 - `optional : <condition>` : Indicate that the register is created only if the _condition_ is true or different from 0.
 - `optional_acc: [na|ro|rw]`: Control the software access (ro) of register which are due to parameters value. Default is `na` (any access will generate a bus access error).

It is also possible to create an array of register by specifying `- <regname>[<arraysize>]`.
The address is auto-incremented for each register instance.
Note that if the register is part of a group, all the register must be instantiated with the same array size.

It is possible to override properties of array element (register or fields). The array index can be a single value (`[0]`), a comma separated list (`[1,5]`) and/or a range (`[0:3]`).

Here is an example with a register array and various override:
```yaml
    instances:
      - version   @ 0x00
      - cfg[$NUM_PINS] = cfg
        description:  GPIO[$i]  configuration register
        [0,5,6].pull_mode.reset = 2
        [0:6,12:14].clk_en.disable = 0
```

## Extra information
In almost every context (rif, rifmux, register/field definition and instance) it is possible to attach
some extra information stored in a dictionary to be used by a custom generator.
This starts with `info:` and then each line indented by one level below this should have the format `- <key> : <value>`
where _key_ is a single word and _value_ is a quoted or unquoted string. Value can be omitted, in which case it is set to True


# RIF Mux syntax

The RIF Mux allows to instantiate multiple RIF with a given address offset for each.

It always starts with `rifmux: <name>`

The possible properties, indented by one level compare to the rifmux declaration, are:

 - `dataWidth <data_width>` : Number of bits of the data bus
 - `addrWidth <addr_width>` : Number of bits of the address bus (byte aligned). Should be large enough to address all instantiated register (i.e. ceil(log2(nb_reg * data_width/4)))
 - `interface : <ifname>` : Define the type of interface used to control the RIF Mux. Possible value are default, apb, uaux.
  By default uses a memory like interface (with a done signal asserted when access is complete).
 - `parameters : `: Start a parameter list. See [paragraph Parameters](#Parameters).
 - `generics : `: Start a generic list. See [paragraph Generics](#Generics).
 - `top` : Specify the top-level RIF component name
 - `map:` : start the mapping of RIFs in the memory space


Each entry of the map is indented by one level compare to the map keyword and follow the syntax:
`- <rif_name> = <rif_type> @ <rifAddr> ["short description"]`

Every RIF definition should be either in the same directory as the rifmux or in the include path list define on the command line.

For each RIF instance it is possible to override :
 - the description using `description : ...`.
 - parameters value with a `parameters:` section using the same syntax as the RIF (cf. [Parameters](#Parameters)).
 - suffix name using `suffix : <suffix_info>` to add a suffix to the file generated (useful when using parameters different from default)
 - `optional : <condition>` : Indicate that the component is instantiated only if the _condition_ is true.
  The condition should be a valid arithemtic expression where parameters can be used: this supports standard math operation (+,-,\*,\%,<<,>>) or comparison operator (==,!=,>,<,...).
  The condition can also simply the name of a generic.
  If the condition is set to enable, this add ports on the RIFmux with a structure of enable for each RIF instance. When the corresponding enable signal is low,
  the rif_en signal of the corresponding RIF instance is forced to 0.


You can also use external RIF (to access memory-like block) with the syntax:
`- <rif_name> external <addr_width> @ <rifAddr>` where _addr_width_ is the range of address that can be addressed in number of bits.

The address can be made relative:
 - Using `@+` instead of `@` uses the rifAddr as an offset to the previous absolute address
 - Using `@+=` is the same, except it also update the previous absolute address

### Example
```yaml
rifmux: soc_rifmux
  addrWidth:  20
  dataWidth:  32
  parameters :
    - AON_ADDR_BASE = 0xC0000
    - MDM_ADDR_BASE = 0xD0000
  interface : default
  map:
    group: AON @ $AON_ADDR_BASE  "Always-On Power domain"
      - pmu_aon    = pmu_aon      @ 0x0000
      - ccu_aon    = ccu_aon      @ 0x1000
        parameters:
          - HAS_LF_XOSC = True
    group: MDM @ $MDM_ADDR_BASE  "Modem Power domain"
      - mdm0          = modem       @ 0x0000
        description : Modem 0 with high speed support
        parameters:
          - tx.HIGH_SPEED = True
          - rx.HIGH_SPEED = True
      - mdm1          = modem       @+= 0x200
        description : Modem 1
        suffix: ctrl=dmi(alt)
      - mdm2          = modem       @+= 0x200
        description : Modem 2 (low power)
        parameters:
          - rx.LOW_POWER = True
    // Analog control, not part of any group
    - ana_fe = ana_fe @ 0xF0000
```