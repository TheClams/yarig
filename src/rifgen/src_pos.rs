use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy, Debug, Default)]
pub struct DeclLine {
    pub decl_line: Option<usize>,
}

impl PartialEq for DeclLine {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// Dictionnary of Source line associated to a property
#[derive(Clone, Debug)]
pub struct PropLines<P>(HashMap<P, usize>);

impl<P> Default for PropLines<P> {
    fn default() -> Self {
        PropLines(HashMap::new())
    }
}

impl<P> Deref for PropLines<P> {
    type Target = HashMap<P, usize>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<P> DerefMut for PropLines<P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Dictionnary of Source (start, end) line range associated to a property
#[derive(Clone, Debug)]
pub struct PropBlocks<P>(HashMap<P, (usize, usize)>);

impl<P> Default for PropBlocks<P> {
    fn default() -> Self {
        PropBlocks(HashMap::new())
    }
}

impl<P> Deref for PropBlocks<P> {
    type Target = HashMap<P, (usize, usize)>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<P> DerefMut for PropBlocks<P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Identifies which description-style block a `PropBlocks` entry tracks. Shared across
/// `Rif`/`RegDef`/`Field`/`RegOverride`'s src-info: each owner only ever populates the subset
/// that applies to it (e.g. `Rif`/`RegOverride` never populate the `Intr*` variants, which only
/// derived-interrupt registers/fields have).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DescBlockKind {
    Public,
    Private,
    IntrEnable,
    IntrMask,
    IntrPending,
}
