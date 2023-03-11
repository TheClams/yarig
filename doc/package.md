# RTL Package & Module

When generating the RTL (SystemVerilog & VHDL) for a RIF, two files are created:

 - a module/entity containing all the logic for address decoding and the registers storage.
 - a package with structures/record for all the register type described in the RIF file.

---
## Package
Each register is split in two structures / record:

 - the software one (suffixed by \_sw_t) driven by the RIF,
 - the hardware one (suffixed by \_hw_t) driven by the rest of the design.

The name of the structure is based on the register name: register REGNAME gives two structure *regname_sw_t* and *regname_hw_t*.

The description property of each field is used as a comment near each field.

#### Register group
Multiple register can be grouped to be create only one pair of structure sw/hw.
This is done at the register declaration, by adding `(regGroupName)` just before the shorst description
to group all registers under the same name regGroupName.

#### Field Type
By default all field have the type *logic* / *std_logic* with the width defined in the RIF.
Adding the property `signed` inside a field allows to change the type to *logic signed*.

#### Special fields
Some structure field are automatically added based on properties defined in the RIF. Here is the convention used.

Property `external` in a register:

 - ext_read (sw): High during a read access on this register
 - ext_write (sw): High during a read access on this register
 - ext_done (hw): high when access (read or write) is done

If a register group is used there will be one ext_regName_* signal per register of the group

Register properties `wrPulse`, `rdPulse`, `accPulse` creates a signal named respectively *p_read*, *p_write* or *p_access* in the software structure:
this signal is asserted for one cycle when the register is accessed for, respectively, read, write or both.
If a register belong to a register group and those properties are used more than one in the group, the register nam is appended to the signal name.

Field properties `hwclr` and `hwset`  simply add a suffix to the fieldname in the hardware structure.

Field Properties `we` and `wel` add a field in the hardware structure with the name fieldname_we or fieldname_wel.

A name can be specified directly after the property:

 - this can be a signal name, declared in the signal, which will be added to the top interface
 - an existing field in the register
 - an undeclared field in the register (using the syntax regName.mywe) and in this case the field will be added to the hardware structure.


---
## Module I/O

#### Clock
By default there is only one clock name is *clk*.
There is two kind of clock internally: software (only one possible) and hardware (multiple are allowed for different hardware driven register), but both should synchronous even if frequency/gating is different. Any clock domain crossing should be handled outside of the RIF.

The software clock name can be changed via `swClock : <sw_clock_name>` in the rif properties (alongside addrWidth, ...).
The hardware clocks are declared via `hwClock : <hw_clk0> <hw_clk1> ...` : by default the first clock will be used for all hardware driven register, but any clock declared (hardware and software) can be used by a register using `clock <clock_name>` in the register property.

#### Reset
There is two kind of reset:

 - "Software Reset" which control all register that can be written by software. By default it is named *rst_n* and can be changed with the syntax `swReset <sw_rst_name>`
 - "Hardware Reset" which control all register written exclusively by hardware. By default it is also named *rst_n*. It can be changed with the syntax `hwReset <hw_rst_name>` either at the page level, affecting all registers or at the field level to override the default register.

By default the reset signal is asynchronous and active low but this can be changed with the following syntax when declaring the signal:
`swReset|hwReset : <rst_name> activeLow|activeHigh async|sync`

#### Register
All registers (or group of registers) with field driven by the RIF generate an output named *rif_reg_instance_name* and the type *regType_sw_t*.
All registers (or group of registers) with field driven by the hardware generate an input named *reg_instance_name* and the type *regType_hw_t*.

By default the *reg_instance_name* of a group of register is the *groupName* of the register type.
This can be changed by adding `(instanceGroupName)` just before the address information (if any).
If there are multiple instances of group of registers it is mandatory to provide a *groupName* for each of the register instance

### RIF interface
The default interface to access the register is using a simple interface similar to the one used by memories.

The interface, named rif_if, has two parameters automatically set by the RIF property:

 - W_ADDR: Number of bits of the address bus, set by the property addrWidth in the RIF.
 - W_DATA: Number of bits of the data bus, set by the property register.size in the RIF.


The following fields are driven by the CPU:

 - en         : High for register access. Last one clock cycle per access even if done takes more than one clock cycle to be asserted
 - rd_wrn     : High for read, low for write (valid when en is high)
 - addr       : Address on W_ADDR bits
 - wr_data    : Write data on W_DATA bits


The following fields are driven by the RIF:

 - done       : Pulse high when access is complete
 - rd_data    : Read data on W_DATA bits, only valid when done is high
 - err_addr   : Qualifier of done signal, high if access failed due to invalid address
 - err_access : Qualifier of done signal, high if access failed due to rd/wr access status


There is two cases to consider:

 - Fixed latency: access can be piped meaning that it is possible to do back-to-back access to different address without waiting for the done to be asserted. This is the case for all internal register generated by RifGen.
 - Mixed latency: it is possible to mix register access that have different latencies (typically when some register are defined as external and they cross domain crossing or access shared ressources with random latency) but in this case enable can be asserted only after the done signal is asserted to preserve access order.

Here are a few waveform example:

```wavedrom
{head:{text:'Access with no latency',tick:0,},
signal: [
  {name: 'clk',          wave: 'p........'},
  {name: 'en',           wave: '01010.1.0'},
  {name: 'rd_wrn',       wave: '010....10'},
  {name: 'addr',         wave: 'x2x3x.45x', data: ['A0', 'A1', 'A2', 'A3']},
  {name: 'wr_data',      wave: 'x..3x.4x.', data: [ 'D1', 'D2']},
  {} ,
  {name: 'done',         wave: '01010.1.0'},
  {name: 'rd_data',      wave: 'x2x...x5x', data: ['D0', 'D3']},
  {name: 'error_access', wave: '02030.450'},
  {name: 'error_addr'  , wave: '02030.450'},
]}
```

```wavedrom
{head:{text:'Access with fixed latency (e.g. 1 clock)',tick:0,},
signal: [
  {name: 'clk',          wave: 'p.........'},
  {name: 'en',           wave: '01010.1.0.'},
  {name: 'rd_wrn',       wave: '010....10.'},
  {name: 'addr',         wave: 'x2x3x.45x.', data: ['A0', 'A1', 'A2', 'A3']},
  {name: 'wr_data',      wave: 'x..3x.4x..', data: [ 'D1', 'D2']},
  {} ,
  {name: 'done',         wave: '0.1010.1.0'},
  {name: 'rd_data',      wave: 'x.2x...x5x', data: ['D0', 'D3']},
  {name: 'error_access', wave: '0.2030.450'},
  {name: 'error_addr'  , wave: '0.2030.450'},
]}
```

```wavedrom
{head:{text:'Access with mixed latency',tick:0,},
signal: [
  {name: 'clk',          wave: 'p...........'},
  {name: 'en',           wave: '01010.10.1.0'},
  {name: 'rd_wrn',       wave: '010......10.'},
  {name: 'addr',         wave: 'x2x3x.4x.52x', data: ['A0', 'A1', 'A2', 'A3','A4']},
  {name: 'wr_data',      wave: 'x..3x.4x..2x', data: [ 'D1', 'D2','D4']},
  {} ,
  {name: 'done',         wave: '0.1.0...1..0'},
  {name: 'rd_data',      wave: 'x.2x...x.5x.', data: ['D0', 'D3']},
  {name: 'error_access', wave: '0.230...4520'},
  {name: 'error_addr'  , wave: '0.230...4520'},
]}
```

The APB interface is also supported, just use `interface : apb` in the RIF properties (same place as the address width and register width).

