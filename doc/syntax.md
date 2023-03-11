# RIF syntax

The syntax for the RIF is inspired by YAML: a human readable text file with structure based on indentation.

It is composed of different levels:

 - [Top](#Top) : Top level structure, only one per file, contains all parameters and signals
 - [Page](#Page): group of register and register instance, with a base address
 - [Registers](#Register): Register declaration, regrouping field
 - [Field](#Field) : N bits inside a register
 - [RegisterInstance](#Registerinstance): Instance name of a register with a given address



---


## Top
The component regroups some top level information about the RIF, definition of signals and parameters.
Syntax is `rif: rifName` where rifName is the name which be used for the RTL module name as well as the base filename of all outputs.

The possible properties, indented by one level compare to the rif declaration, are:

 - `addrWidth : <addrWidth>` : Number of bits of the address bus (byte aligned)
 - `dataWidth : <dataWidth>` : Number of bits of the data bus
 - `interface : <ifname>` : Define the type of interface used for the RIF. Possible value are default, apb, uaux.
 	By default uses a memory like interface (with a done signal asserted when access is complete).
 - `description : <blabla>` : Provides some high level information.  Mainly for documentation (e.g. HTML output). Quotation mark are optional and removed for the first line. Description can be on multiple lines as long as it is indented by at least one level compare to the keyword description
 - `swClock : <clock_name>` : Name the software clock signal _clock_name_ (clk by default)
 - `hwClock : <clock_name0> <clock_name1> ...` : Declare hardware clocks. The first one will be the default one.
 - `swReset|hwReset : <rst_name> [activeLow|activeHigh] [async|sync]`: Specify reset signal for software/hardware.
 - `swClkEn|hwClkEn : <clk_en_name> `: Specify default clock enable signal for software/hardware clocks. This can be overriden on a register basis
 - `swClear : <clear_name>` : Declare a global clear _clear_name_ active high to clear all software register (no clear by default)
 - `hwClear : <clear_name0> <clear_name1> ...` : Declare global hardware clears _clear_name_ active high to clear all hardware register (no clear by default). There should be as many signal declared as there are hwClock: same name can be repeated and the minus character `-` can be used indicate that there is no clear for the corresponding hwClock.
 - `parameters : `: Start a parameter list. See [paragraph Parameters](#Parameters).
 - `generics : `: Start a genric list. See [paragraph Generics](#Generics).
 - `- pageName : [description]` : Start a page named pageName. See [below](#Page). The name is used only inside the documentation.

The only mandatory property is at least one page.
By default the address is on 16b, the data on 32, the clock is named clk,
the reset is rst_n (asynchronous, active low) and used for both hardware and software logic.


## Parameters

A parameter is pair name/value declared with `- name = value`.

Declaration of parameter can use arithmetic operation : `-param1 = 2*param2-1`.
It supports all basic arithmetic operation and a few functions like pow, log2, log10, ceil and floor.
Note that pow, log2 and log10 output is a floating point number and should typically be associated with ceil/floor operation when the parameter value is used for field width or array size.

Any occurence of _$parameterName_ in the RIF will be replaced by its value.

Parameters value can be overriden by a command-line argument `--param name value`

## Generics

A generic is defined as `- name = [min:]<default>:<max>`.

This generate an RTL with input parameter.
The generic value can only be used for register instances array size.
If no minimum value is provided, it is set to 1.


## Page
The page contains the definition of n [registers](#Register) , their [register instances](#Registerinstance) and a few page properties.

The available options, indented by one level compare to the page, are:

 - `baseAddress <offset>` : Address offset of all register inside the page. Format can be decimal (64) or hexadecimal (0x20)
 - `addrWidth <nb_bit>` : Define the page range in Number of bits. Format can be decimal (64) or hexadecimal (0x20). This is only required for external pages
 - `clkEn : <clock_enable_name>` : Define a clock enable signal for all register in the page
 - `external` : Indicates that the page logic is external. The only logic provided will be the address decoding.
 - `optional : <condition>` : Indicate that the register of a page are instantiated only if the _condition_ is true.
 	The condition should be a valid python arithemtic expression where parameters can be used.
 - `registers:` : Start the register declaration entry. See [below](#Register) for detail.
 - `instances:` : Start the register instances entry. See [below](#Registerinstance) for detail.
 - `include <rifName>.<pageName>` : Include a page from another RIF, both register definition and instance. This cannot be mixed with manual registers/instances entry !


## Register
The register is a group of fields with some basic properties.

Its declaration is indented by one level compare to the registers keyword and follow the syntax:
`- reg_name: [(regGroup)] ["Register short description"]`

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
 - `description.hidden: <extra description` : Extra description displayed only when documentation is generated without the --doc_hide flag
 - `mask|enable.description <long description>` : For interrupt register, set the description of the associated mask and enable register.
 - `external` : The register logic is handled outside of the RIF which provides additional signal related to address decoding.
 - `externalDone` : The register logic for the done access is handled outside of the RIF, allowing to add wait state.
 - `wrPulse|rdPulse|accPulse [comb|reg] [clockName]` : A pulse is generated when the register is, respectively written, read or both.
 	The pulse can be combinatorial (i.e. the pulse happens at the same time as the register access) or registered (i.e. one clock latency).
 	By default, the read pulse is combinatorial while the write/access are registered.
 	When registered, a clock can be provided to indicate a different clock than the register one to generate the pulse.
 - `interrupt high|low|rising|falling|edge [en[=valEnable]] [mask[=valMask]] [pending] [rclr|wclr|w0clr|w1clr|hwclr]` : Indicates that all fields in the register are interrupts, active either on a level (high or low) or an edge (rising, falling or both).
 The optional enable property will auto declare a register to enable the interrupt with same name as the interrupt register plus \_en with a reset value _valEnable_ (for the whole register). Every hardware interrupt goes through an and of the enable register before updating the status register.
The optional mask property will auto declare a register to mask the interrupt with same name as the interrupt register plus \_mask with a reset value _valMask_ (for the whole register). The interrupt request signal is the OR of an AND between the status register and the mask register. So, when the value for a field is 0, the corresponding interrupt status is asserted but this does not trigger the interrupt request.
  The optional pending property (only allowed if a mask is used) will auto declare a read-only register containing the AND of the status and the mask (which was used to generate the external interrupt request signal).
 Default to level high, with clear on read with no mask, enable or pending if no information is provided.
 - `optional : <condition>` : Indicate that the register is defined only if the _condition_ is true.
  The condition should be a valid python arithemtic expression where parameters can be used.
 - `- fieldName ...` : Define a fields named _fieldName_ inside the register. See [below](#Field) for detail.

It is also possible to include the definition from another rif file using the following syntax:

 - `- include <rifName>.*` : Include all register definition
 - `- include <rifName>.<pageName>.*` : Include all register defined in a page
 - `- include <rifName>.<regName>` : Include only one register


## Field
The field is group of bits inside a registers. The casing convention is snake_case in the rif: it will be lowered
case and used in the RTL output (including the RAL), and converted to camelCase in the C and documentation (optionnaly PascalCase).

Its declaration is indented by one level compare to the register declaration and follow the syntax:
`- field_name [= resetValue]  msb:lsb [ro|rw|rclr|w1clr|w0clr|w1set] ["Field short description"]`
The field position can also use the systemVerilog range syntax `lsb+:width` instead of `msb:lsb`

The general default behavior is:
 - If a register can be read/write by the software then the hardware access is in read-only
 - If the register is in read-only for the software then the hardware access is write-only
 - If both software and hardware can modify a register value, then the hardware access needs
   some mechanism to indicate when its value should be used to modify the register value (via we/wel/hwset/hwclr/hwtgl)

The optional properties of a field, indented by one level compare to the field declaration, are:

 - `description: <long description>` : More detailed register description. Mainly for documentation (e.g. HTML output). Quotation mark are removed on the first line. Description can be on multiple lines as long as it is indented by at least one level compare to the keyword description
 - `description.hidden: <extra description` : Extra description displayed only when documentation is generated without the --doc_hide flag
 - `mask|enable|pending.description <long description>` : For interrupt register, set the description of the associated mask/enable/pending register field. Again, description can be done on multiple lines.
 - `hw [r|w|rw|na]` : Specify a type of access from the hardware
 	- `r` : read-only (Default)
 	- `w` : write-only
 	- `rw` : read/write
 	- `na` : Not accessible from HW
 - `clkEn <clock_enable_name` : Define a clock enable signal for this field
 - `hwset [setSignal] [dataSignal]` : Hardware high set the field to 1.
 	The _setSignal_ can be in the form `self.field_name` in which case a field is added to the hardware structure of the register (if it doesn not exists).
 	If _setSignal_ is in the form `reg_name.signal_name`, then it references a field in another register structure.
  If _setSignal_ is simply of the form `signal_name`, then a port with this name is added.
  If _setSignal_ starts with a dot like `.signal_name`, then it reference an internal signal of the module (for example the address decode signals).
 	If no _setSignal_ is provided, a field with the name _reg_name_hwset_ is automatically added.
 	When used on multi-bit field a _dataSignal_ must be provided: each bit high will set the corresponding bit high in the field when the _setSignal_ goes high.
 - `hwclr [clrSignal] [dataSignal]` : Hardware high set the field to 0.
 	If no _clrSignal_ is provided, a field with the name _reg_name_hwclr_ is automatically added.
 	When used on multi-bit field if no _dataSignal_ is provided the whole field is set to 0. Otherwise only bits high in _dataSignal_ are reset to 0 when _setSignal_ goes high.
 - `hwtgl [tglSignal] [dataSignal]` : Hardware high toggle the field value.
 	If no _tglSignal_ is provided, a field with the name _reg_name_hwtgl_ is automatically added.
 	When used on multi-bit field if no _dataSignal_ is provided the whole field is inverted. Otherwise only bits high in _dataSignal_ are toggled _setSignal_ goes high.
 - `lock [lockSignal]` : Signal to prevent a register to be written. _lockSignal_ follow the same rule as _setSignal_ just above.
 - `pulse [comb]` : The field stays high only one cycle after being set. If pulse is followed by `comb`, then the pulse is generated on the write signal without extra flop in the block.
 - `toggle` : When a 1 is written by software on this field, the field inverts its value.
 - `swset`  : Software can only set bits of the field to 1.
 - `signed` : Indicates that the value stored is a signed value.
 - `we [weSignal]` : The copy from hardware to the RIF field value is done only when a write enable signal is high. Valid only for field with write access from hardware.
 - `wel [welSignal]` : Same as 'we' but write enable signal is active low.
 - `clear [clearSignal]` : Synchronous clear, setting the field to its reset value. _clrSignal_ follow the same rule as the hwset/hwclr/hwtgl.
 	This property is slightly different from the hwclr: first the field is set to its default reset value instead of just 0
 	and second the clear signal bypass the clock enable
 - `counter up|down|updown [incrVal[=width]] [decrVal[=width]] [clr] [sat]`: The field is a counter up and/or down, with optional input signal for the increment/decrement value and the clear.
  By default the increment/decrement value signals have the same width as the counter but this can be changed by specifying a number after the incr/decrVal keyword.
 	Note: the counter value is not accessible by default from hardware, but this this can be changed with `hw rw` property which must be placed after the counter declaration.
 - `partial <lsb_pos>` : Indicates that this field is larger than the register and that the LSB correspond to the bit _lsb_pos_ of larger field.
 - `interrupt high|low|rising|falling|edge [en=<valEnable>] [mask=<valMask>] [rclr|wclr|w0clr|w1clr|hwclr]` : override default interrupt settings.
 - `limit ([min:max]|{v0,v1,..}|enum) [bypass_signal]` : Limit valid write value for a field. The limit can be a range in the form of `[min:max]` (min & max included, one can be omitted), a set of value with `{v0,v1,v2}` or, when the field is an enum using the keyword `enum` will automatically limit the value to the enumerated values. When writing an invalid value, the register will not be updated and an access error will be raised on the control bus. If a bypass signal is provided, when the signal is high the limit is ignored.
 - `password [once=<val>] [hold=<val>] [protect]` : The field is used to generate an internal signal named `<regname>_<fieldname>_locked` which is initialize to 1
  and reset to 0 if the written value match one of the password value. If the password correspond to once the field will stay low until the next write.
  Otherwise it stays low until a value different from the a valid code is written.
  The optional `protect` will lock the password until the next reset if a wrong value (except 0) is written.
  This field has automatically the property `hidden` (i.e. won't appear in documentation generated with --doc_hide.
  The read value correspond to the state of the password: 1 for locked, 0 for unlocked once, 2 for unlocked hold and 3 for stucked (i.e. need reset).
 - `hidden` : allows to hide the field in any documentation (HTML, MIF, C Header) if the flag --doc_hide is enabled
 - `disable` : force the field to the reset value. Used typically when overloading included register.
 - `reserved` : Renamed field in any documentation to rsvdxx (where xx is the LSB) when the flag --doc_hide is enabled. Also remove description.
 - `optional : <condition>` : Indicate that the field is defined only if the _condition_ is true.
  The condition should be a valid python arithemtic expression where parameters can be used.

Some field can be declared as enum, with description of each possible value.
This starts by adding the property enum with `enum : [type]`. If the keyword type is provided a typedef enum will be declared with the type name `e_regname_fieldname`. The type name can be forced by providing any valid identifier, and if a package scope is provided in the type name, no typedef will be declared, but the field will have the poper type.
It is then followed by n line indented, following the syntax `- name = value "description"`.

A field can also be declared as array by simply giving the array size with the fieldname: `- fieldName[arraySize] ...`
The position of the field corresponds to the position of the field at index 0.
The others are auto-instantiated, with a position that increment by the size of the fields by default.
To change the increment use the option `arrayPosIncr value`.
It is also possible to have an array that extends on more than one register: this works like the partial field, with the keyword `arrayPartial offset`.
In the description, the field index can be referenced by `$i` or even in a formula such as `${2*i+1}`.

## RegisterInstance
If all the register declared have to be instantiated just once, in order, the simplest way is to simply use `instance: auto`.

If the auto keyword is not used, then all instance of register are added with the following syntax,
indented by one level compare to the instances keyword:
`- reg_name [= regType] [(groupName)] [@ regAddr]`

By default the register type is the same as the register name, and the address is auto incremented compare to the previous register.

The address is always aligned on byte, no matter what the address width.

The groupName is only neccessary when there is multiple instances of register belonging to a register group.

It is possible to override some properties at the instance level. Here the syntax to use, indented by one level:

 - `description : <bla bla>` : register description, can be multi-line as long as it is indented by another level. Quotation mark on first line are removed.
 - `hw r|w|rw|na` : change the hardware access of a register
 - `field_name.description <bla bla>` : change a field description
 - `field_name.reset = <rst_val>` : change a field reset value

It is also possible to create an array of register by specifying `regname[arraysize]`.
The address is auto-incremented for each register instance.
Note that if the register is part of a group, all the register must be instantiated with the same array size.

## Extra information
In almost every context (rif, rifmux, register/field definition and instance) it is possible to attach
some extra information store in a dictionary to be used by a generator.
This starts with `info:` and then each line indented by one level below this should have the format `- <key> : <value>`
where _key_ is a single word and _value_ is a quoted or unquoted string. Value can be ommited, in which case it is set to True


# RIF Mux syntax

The RIF Mux allows to instantiate multiple RIF with a given address offset for each.

It always starts with `rifmux: rifmuxName`

The possible properties, indented by one level compare to the rifmux declaration, are:

 - `addrWidth <addrWidth>` : Number of bits of the address bus (byte aligned). Should be bigger than
 - `dataWidth <dataWidth>` : Number of bits of the data bus
 - `interface : <ifname>` : Define the type of interface used to control the RIF Mux. Possible value are default, apb, uaux.
 	By default uses a memory like interface (with a done signal asserted when access is complete).
 - `map:` : start the mapping of RIFs in the memory space


Each entry of the map is indented by one level compare to the map keyword and follow the syntax:
`- <rif_name> = <rifType> @ <rifAddr> ["short description"]`

Every rif definition should be either in the same directory as the rifmux or in the inc_rif path define on the command line.

For each RIF instance it is possible to override :
 - the description using `description : ...`.
 - parameters value with a `parameters:` section using the same syntax as the RIF (cf. [Parameters](#Parameters)).
 - suffix name using `suffix : <suffix_name>` to add a suffix to the file generated (usefull when using parameters different from default)

You can also use external RIF (to access memory-like block) with the syntax:
`- <rif_name> external <addrWidth> @ <rifAddr>` where _addrWidth_ is the range of address that can be addressed in number of bits.

The address can be made relative:
 - Using `@+` instead of `@` uses the rifAddr as an offset to the previous absolute address
 - Using `@+=` is the same, except it also update the previous absolute address
