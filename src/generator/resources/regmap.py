from collections.abc import Generator
from enum import IntEnum
import typing

# pyright: reportAny = false

class Peripheral(object):
    '''address and collection of registers'''

    def __init__(self, name: str = '', addr: int = 0) :
        self.__addr__ : int = addr
        self.__name__ : str = name

    def rifs(self) -> Generator['Peripheral']:
        '''Iterator over all rifs'''
        for attr_name in vars(self):
            obj = getattr(self, attr_name)
            if isinstance(obj, Peripheral):
                yield obj

    def regs(self) -> Generator['Register']:
        '''Iterator over all registers'''
        for attr_name in vars(self):
            obj = getattr(self, attr_name)
            if isinstance(obj, Register):
                yield obj

    def address(self) -> int:
        '''Address of the peripheral'''
        return self.__addr__

    def name(self) -> str:
        '''Instance name of the peripheral'''
        return self.__name__

    def by_name(self, path: str) -> 'None|Peripheral|Register|Field':
        '''Retrieve an element or sub-element of the peripheral by name
        Accept path such as 'rif.reg.field' '''
        obj = self
        for n in path.split('.'):
            o = getattr(obj, n)
            if isinstance(o, (Peripheral,Register,Field)):
                obj = o
            else:
                return None
        return obj


class Register(object):
    '''Register definition: address and collection of fields'''

    @typing.final
    class regInfo:

        def __init__(self, parent: None | Peripheral, name: str, address: int, readonly: bool):
            self.parent = parent
            self.name = name
            self.address = address
            self.readonly = readonly
            self.flags : list[str] = []

    def __init__(self, parent: None | Peripheral, name: str, addr: int, readonly: bool, init: None|int = None) :
        self.__reg_info__ : Register.regInfo = Register.regInfo(parent, name, addr, readonly)
        if init is not None:
            self.set_value(init)

    @typing.override
    def __repr__(self) -> str:
        s = f'{self.__reg_info__.name} = 0x{self.get_value():08x}'
        for f in self.fields():
            s += f'\n\t{f.name} = {f}'
        return s

    def fields(self):
        '''Iterator over all fields'''
        for attr_name in vars(self):
            obj = getattr(self, attr_name)
            if isinstance(obj, Field):
                yield obj
            elif isinstance(obj, dict):
                for _, x in obj.items():
                    if isinstance(x, Field):
                        yield x

    def parent(self) -> None | Peripheral:
        '''Return peripheral which owns the register'''
        return self.__reg_info__.parent

    def name(self) -> str:
        '''Instance name of the register'''
        return self.__reg_info__.name

    def readonly(self) -> bool:
        '''Return true when register is read-only'''
        return self.__reg_info__.readonly

    def flags(self) -> list[str]:
        '''Register flags: interrupt, external, ...'''
        return self.__reg_info__.flags

    def by_name(self, name: str) -> 'None|Field':
        '''Retrieve a field by name'''
        obj = getattr(self, name)
        return obj if isinstance(obj, Field) else None

    def address(self) -> int:
        '''Absolute address of the register'''
        if self.__reg_info__.parent is not None:
            return self.__reg_info__.parent.__addr__ + self.__reg_info__.address
        return self.__reg_info__.address

    def set_value(self, value_all_fields: int):
        '''Set value for the register'''
        for field in self.fields():
            reg_val = value_all_fields >> field.pos
            mask = (1<<field.width) - 1
            reg_val &= mask
            field.value = reg_val

    def get_value(self) -> int:
        '''Get value for the register'''
        reg_val = 0
        for field in self.fields():
            mask = (1 << field.width) - 1
            reg_val |= (field.value & mask) << field.pos
        return reg_val

class Field(object):
    '''field value, width and position inside a register'''

    value  : int = 0
    width  : int = 0
    pos    : int = 0
    name   : str = ""
    kind   : str = ""
    signed : bool = False
    nb_frac: int = 0

    def __init__(self, parent: Register, name: str = '', init: None|int = None) :
        self.__parent__ : Register = parent
        self.name = name
        if init is not None:
            self.value = init

    @typing.override
    def __repr__(self) -> str:
        kind = self.enum_kind()
        v = self.get_value()
        if kind is not None:
            s = f'{kind(v).name} ({v})'
        else:
            if self.nb_frac != 0:
                s = f'{self.get_float()} ({v})'
            else :
                s = f'{v}'
        return s

    def parent(self) -> Register:
        '''Return register owning the field'''
        return self.__parent__

    def mask(self) -> int:
        '''Return mask corresponding to the field in the register'''
        return ((1<<self.width) - 1) << self.pos

    def set_value(self, value: int | float):
        '''Set the field value, either as internal integer representation or the corresponding fixed-point value'''
        if isinstance(value, int):
            self.value = value
        else :
            conv = getattr(self.enum_kind(), 'from_float', None)
            if conv is not None:
                self.value = conv(value)
            elif self.nb_frac < 0:
                self.value = int(value / (1 << (-self.nb_frac)))
            else :
                self.value = int(value * (1 << self.nb_frac))

    def get_value(self) -> int:
        '''Return the field value (internal integer representation)'''
        v = self.value
        if self.signed and v > 1<<(self.width-1):
            v -= (1<<self.width)
        return v

    def get_float(self) -> float:
        '''Return the field value (fixed-point representation)'''
        v = float(self.get_value())
        if self.nb_frac < 0:
            v = v * (1 << (-self.nb_frac))
        else :
            v = v / (1 << self.nb_frac)
        return v

    def address(self) -> int:
        '''Return address of the register owning the field'''
        if self.__parent__ :
            return self.__parent__.address()
        return 0

    def enum_kind(self) -> None| type[IntEnum]:
        return None
