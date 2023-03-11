#![macro_use]
#![allow(unused_macros)]
use std::{error::Error, fmt::Display};

use crate::rifgen::context::Context;

pub struct ErrorContext {
    pub line_num: usize,
    pub cntxt: Context,
}

impl ErrorContext {
    pub fn new() -> ErrorContext {
        ErrorContext { line_num: 0, cntxt: Context::Top }
    }
    pub fn set(&mut self, line_num: usize, c: Context) {
        self.line_num = line_num;
        self.cntxt = c;
    }
    #[allow(unused)]
    pub fn set_cntxt(&mut self, c: Context) {
        self.cntxt = c;
    }
}

thread_local!(pub static ERROR_CONTEXT: std::cell::RefCell<ErrorContext>  = std::cell::RefCell::new( ErrorContext::new() ) );
macro_rules! err_context_set {
    ($n:expr, $c:expr) => {{ ERROR_CONTEXT.with(|e| {e.borrow_mut().set($n,$c)}) }};
    ($c:expr) => {{ ERROR_CONTEXT.with(|e| {e.borrow_mut().set_cntxt($c)}) }};
}


#[allow(dead_code)]
#[derive(PartialEq, Clone, Debug)]
pub enum RifErrorKind {
    /// File IO error
    Io,
    /// Parsing error
    Parse,
    /// Field Kind incompatibility
    FieldKind,
    /// Interrupt setting in non interrupt register
    NotIntr,
    /// Missing register definition
    MissingDef,
    /// Unsupported Feature
    Unsupported,
    /// Duplicated register/field definition
    Duplicated,
    /// Generic errror
    Generic,
}

#[derive(Debug, PartialEq)]
pub struct RifError {
    pub kind: RifErrorKind,
    pub line_num: usize,
    pub txt: String,
}

impl Error for RifError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl From<std::io::Error> for RifError {
    fn from(cause: std::io::Error) -> RifError {
        RifError{
            kind:RifErrorKind::Io,
            line_num: 0,
            txt: format!("{cause}")
        }
    }
}

impl From<winnow::error::ParseError<&str, winnow::error::ContextError>> for RifError {
    fn from(cause: winnow::error::ParseError<&str, winnow::error::ContextError> ) -> RifError {
        RifError{
            kind:RifErrorKind::Parse,
            line_num: ERROR_CONTEXT.with(|c| c.borrow().line_num),
            txt: format!("Unable to parse {} elements\n{}", ERROR_CONTEXT.with(|c| c.borrow().cntxt.to_owned()), cause)
        }
    }
}

impl From< winnow::error::ErrMode<winnow::error::ContextError> > for RifError {
    fn from(cause: winnow::error::ErrMode<winnow::error::ContextError> ) -> RifError {
        RifError{
            kind:RifErrorKind::Parse,
            line_num: ERROR_CONTEXT.with(|c| c.borrow().line_num),
            txt: format!("{} | {}", ERROR_CONTEXT.with(|c| c.borrow().cntxt.to_owned()), cause)
        }
    }
}

impl From<RifErrorKind> for RifError {
    fn from(kind: RifErrorKind ) -> RifError {
        RifError{
            kind,
            line_num: ERROR_CONTEXT.with(|c| c.borrow().line_num),
            txt: format!("{}", ERROR_CONTEXT.with(|c| c.borrow().cntxt.to_owned()))
        }
    }
}

impl From<String> for RifError {
    fn from(txt: String ) -> RifError {
        RifError{
            kind: RifErrorKind::Generic,
            line_num: 0,
            txt
        }
    }
}

#[allow(dead_code)]
impl RifError {

    pub fn missing_def(name: &str) -> Self {
        RifError {
            kind: RifErrorKind::MissingDef,
            line_num: ERROR_CONTEXT.with(|c| c.borrow().line_num),
            txt: name.to_owned()
        }
    }

    pub fn unsupported(cntxt: Context, line: &str) -> Self {
        RifError {
            kind: RifErrorKind::Unsupported,
            line_num: ERROR_CONTEXT.with(|c| c.borrow().line_num),
            txt: format!("{} in {} | '{}'", cntxt,  ERROR_CONTEXT.with(|c| c.borrow().cntxt.to_owned()), line)
        }
    }

    pub fn duplicated(cntxt: Context, name: &str) -> Self {
        RifError {
            kind: RifErrorKind::Duplicated,
            line_num: ERROR_CONTEXT.with(|c| c.borrow().line_num),
            txt: format!("{} {}",cntxt, name.to_owned())
        }
    }
}

impl Display for RifError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            RifErrorKind::Io          => write!(f, "IO exception: {}",self.txt),
            RifErrorKind::Parse       => write!(f, "Line {}: {}",self.line_num, self.txt),
            RifErrorKind::FieldKind   => write!(f, "Line {}: incompatible field kind {}",self.line_num, self.txt),
            RifErrorKind::NotIntr     => write!(f, "Line {}: Trying to set interrupt properties while register is not an interrupt",self.line_num),
            RifErrorKind::MissingDef  => write!(f, "Line {}: Missing register definition for {}",self.line_num, self.txt),
            RifErrorKind::Unsupported => write!(f, "Line {}: Unsupported feature {}",self.line_num, self.txt),
            RifErrorKind::Duplicated  => write!(f, "Line {}: {} duplicated !",self.line_num, self.txt),
            RifErrorKind::Generic     => write!(f, "{}", self.txt),
        }
    }
}