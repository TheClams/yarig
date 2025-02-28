import typing

class Peripheral(object):
    '''address and collection of registers'''

    def __init__(self, addr: int = 0) :
        self.base : int = addr

class Register(object):
    '''Register definition: address and collection of fields'''

    flags : list[str] = []
    '''Register flags: interrupt, external, ...'''

    @typing.final
    class regInfo:

        def __init__(self, parent: None | Peripheral, name: str, address: int, readonly: bool):
            self.parent = parent
            self.name = name
            self.address = address
            self.readonly = readonly

    def __init__(self, parent: None | Peripheral, name: str, addr: int, readonly: bool, init: None|int = None) :
        self.__reg_info__ : Register.regInfo = Register.regInfo(parent, name, addr, readonly)
        if init is not None:
            self.set_value(init)

    @typing.override
    def __repr__(self) -> str:
        s = f'{self.__reg_info__.name} = 0x{self.get_value():08x}'
        for f in self.fields():
            s += f'\n\t{f.name} = {f.get_value()}'
        return s

    def fields(self):
        for attr_name in vars(self):
            obj = getattr(self, attr_name)
            if isinstance(obj, Field):
                yield obj

    def parent(self) -> None | Peripheral:
        return self.__reg_info__.parent

    def name(self) -> str:
        return self.__reg_info__.name

    def address(self) -> int:
        if self.__reg_info__.parent is not None:
            return self.__reg_info__.parent.base + self.__reg_info__.address
        return self.__reg_info__.address

    def set_value(self, value_all_fields: int):
        for field in self.fields():
            reg_val = value_all_fields >> field.pos
            mask = (1<<field.width) - 1
            reg_val &= mask
            field.value = reg_val

    def get_value(self) -> int:
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
    signed : bool = False
    nb_frac: int = 0

    def __init__(self, parent: Register, name: str = '', init: None|int = None) :
        self.__parent__ : Register = parent
        self.name = name
        if init is not None:
            self.value = init

    def parent(self) -> Register:
        return self.__parent__

    def mask(self) -> int:
        return ((1<<self.width) - 1) << self.pos

    def set_value(self, value: int | float):
        if isinstance(value, int):
            self.value = value
        else :
            if self.nb_frac < 0:
                self.value = int(value / (1 << (-self.nb_frac)))
            else :
                self.value = int(value * (1 << self.nb_frac))

    def get_value(self) -> int:
        v = self.value
        if self.signed and v > 1<<(self.width-1):
            v -= (1<<self.width)
        return v

    def get_float(self) -> float:
        v = float(self.get_value())
        if self.nb_frac < 0:
            v = v * (1 << (-self.nb_frac))
        else :
            v = v / (1 << self.nb_frac)
        return v

    def address(self) -> int:
        if self.__parent__ :
            return self.__parent__.address()
        return 0
