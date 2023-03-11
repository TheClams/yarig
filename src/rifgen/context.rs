#[derive(Clone, Debug, PartialEq)]
/// Parsing context
pub enum Context {
	/// Top level context (i.e. default context before any non-comment has been parsed)
	Top,
	/// RIF context started with keyword `rif` inside Top context
	Rif,
	/// RIF Mux context, start with keyword `rifmux` inside Top context
    Rifmux,
    /// RIF Mux mapping started by keyword `map`
    RifmuxMap,
    /// RIF Mux Top definition, started with keyword `top`
    RifmuxTop,
    /// RIF Mux group of instances
    RifmuxGroup,
    /// RIF instance properties
    RifInst,
	/// Interface context started with the keyword interface inside Rif or Rifmux context
    Interface,
    /// Indicate how RTL packages handles suffixes (true to use it, false to ignore)
    SuffixPkg,
    /// Configure the suffix to add to a rif instance
    Suffix,
    /// Include context started with include keyword in a page or register context
    Include,
    /// Description context (multiple unquoted strings) started with keyword descr or description in various contexts
    Description,
    /// Description for the interrupt enable started with enable.decsription in a register
    DescIntrEnable,
    /// Description for the interrupt mask started with mask.decsription in a register
    DescIntrMask,
    /// Description for the interrupt pending started with pending.decsription in a register
    DescIntrPending,
    /// List of key/value parameters started with parameter keyword
    Parameters,
    /// List of key/(value/max) generics started with parameter generic
    Generics,
    /// Page properties started by an item name `- page_name : "description"`
    Page,
    ///
    Registers,
    RegDecl,
    Field,
    Enum,
    Instances, RegInst,
    Info,
    AddrWidth,
    DataWidth,
    BaseAddress,
    SwClock, SwClkEn, SwReset, SwClear,
    HwClock, HwClkEn, HwReset, HwClear,
    HwAccess, HwSet, HwClr, HwTgl, HwLock, HwWe, HwWel,
    SwSet, Signed,
	External, ExternalDone,
	RegPulseWr,RegPulseRd,RegPulseAcc,
	Pulse, Toggle,
	Interrupt,
    InterruptAlt,
    Counter,
    Partial,
	Hidden, Disabled, Reserved, ArrayPosIncr, ArrayPartial,
    /// Flag a page/register/field/instance as optional. Followed by a paramter
	Optional,
    /// Set limit of field write value (started by keyword `limit`)
    Limit,
    /// Set limit of field write value (started by keyword `limit`)
    Password,
    /// Generic item : format is `- identifier`
    Item(String),
    PathStart(String),
    RegIndex(u16),
    FieldIndex((String,u16)),
}

impl std::fmt::Display for Context {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
