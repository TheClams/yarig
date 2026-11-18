use crate::parser::{logic_expr, parser_expr::{parse_expr, ParamValues}};

/// Thin wrapper around string to ease update and get short/long descripion
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Description {
    public: String,
    private: String,
}

impl Description {

    /// Add one line of description
    pub fn updt(&mut self, desc: &str, is_private: bool) {
        let s = if is_private {&mut self.private} else {&mut self.public};
        if !s.is_empty() {
            s.push('\n');
        }
        let mut line_it = desc.split("\\n").peekable();
        while let Some(line) = line_it.next() {
            s.push_str(line);
            if line_it.peek().is_some() {
                s.push('\n');
            }
        }
    }

    /// Return public/private description as a string
    pub fn get(&self, is_public: bool) -> String {
        let mut desc = String::with_capacity(self.len(is_public));
        desc.push_str(&self.public);
        if !is_public && !self.private.is_empty(){
            desc.push('\n');
            desc.push_str(&self.private);
        }
        desc
    }

    /// Replace the whole public description.
    pub fn set_public(&mut self, text: &str) {
        self.public = text.to_owned();
    }

    /// Replace the whole private description.
    pub fn set_private(&mut self, text: &str) {
        self.private = text.to_owned();
    }

    /// Return first line of description
    pub fn get_short(&self, is_public: bool) -> String {
        let s = if self.public.is_empty() && !is_public {
            &self.private
        } else {
            &self.public
        };
        if let Some(end) = s.find('\n') {
            s[..end].to_owned()
        } else {
            s.to_owned()
        }
    }

    /// Return description as a pair of short/long description
    pub fn get_split(&self, is_public: bool) -> (String, Option<String>) {
        let d = self.get(is_public);
    	match d.find('\n') {
    		Some(mid) if mid < d.len()-1 => (d[..mid].to_owned(), Some(d[mid+1..d.len()].to_owned())),
    		Some(end)  => (d[..end].to_owned(), None),
    		None => (d, None),
    	}
    }

    /// Check if the description is empty
    pub fn is_empty(&self, is_public: bool) -> bool {
        self.public.is_empty() && (is_public || self.private.is_empty())
    }

    /// Check if the description is empty
    pub fn len(&self, is_public: bool) -> usize {
        self.public.len() + (if is_public {0} else {self.private.len()})
    }

    /// Return description (both public and private) after interpolating the index (if any)
    pub fn interpolate(&self, idx: DescIdx) -> Description {
        let public = Self::interpolate_str(&self.public, idx);
        let private = Self::interpolate_str(&self.private, idx);
        Description {public, private}
    }

    fn interpolate_str(s: &str, idx: DescIdx) -> String {
        let mut desc = String::with_capacity(s.len());
        let params = ParamValues::new_with_idx(idx.val() as isize);
        let mut parts = s.split('$');
        desc.push_str(parts.next().unwrap_or(""));
        for mut s in parts {
            // Variable $i replaced by index
            if let Some(stripped) = s.strip_prefix('i') {
                desc.push_str(&format!("{}", idx.val()));
                desc.push_str(stripped);
            }
            // Replace $[] by [msb:lsb]
            else if let Some(stripped) = s.strip_prefix("[]") {
                match idx {
                    DescIdx::None => {},
                    DescIdx::Array(i)     => desc.push_str(&format!("[{i}]")),
                    DescIdx::RegBus(w, i) => desc.push_str(&format!("[{}:{}]", i * w + w - 1, i*w)),
                    DescIdx::FieldBus(w, _) => desc.push_str(&format!("[i*{w}+{}:i*{w}]", w-1)),
                }
                desc.push_str(stripped);
            }
            // Start of an equation: `$(expr)` or `${expr}`
            else if s.starts_with('(') || s.starts_with('{') {
                let orig = s;
                let (expr_s, rest) = if s.starts_with('{') {
                    match Self::split_braces(s) {
                        Some((e, r)) => (Some(e), r),
                        None => (None, s),
                    }
                } else {
                    match logic_expr(&mut s) {
                        Ok(e) => (Some(e), s),
                        Err(_) => (None, orig),
                    }
                };
                // Replace by the value when the equation is valid, otherwise leave text unchanged
                let val = expr_s
                    .and_then(|e| parse_expr(e).ok())
                    .and_then(|expr| expr.eval(&params).ok());
                match val {
                    Some(val) => {
                        desc.push_str(&format!("{val}"));
                        desc.push_str(rest);
                    }
                    None => {
                        desc.push('$');
                        desc.push_str(orig);
                    }
                }
            } else {
                desc.push_str(s);
            }
        }
        desc
    }

    /// Split a string starting with a curly bracket into (content of the balanced brackets, remainder)
    fn split_braces(s: &str) -> Option<(&str, &str)> {
        let mut depth = 0usize;
        for (pos, c) in s.char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((&s[1..pos], &s[pos + 1..]));
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// Create a description with $f replace by a format string (for example u8.0 or s5.3)
    pub fn with_format(&self, format: &str) -> Description {
        let public = self.public.replace("$f", format);
        let private = self.private.replace("$f", format);
        Description{public, private}
    }

    /// Remove the $ special character from description:
    ///  used when we do not want the interpolated version (for register base description typically)
    pub fn no_dollar(&self) -> Description {
        Description{
            public: Self::no_dollar_str(&self.public),
            private: Self::no_dollar_str(&self.private),
        }
    }

    fn no_dollar_str(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut parts = s.split('$');
        out.push_str(parts.next().unwrap_or(""));
        for part in parts {
            // Remove complete `${...}` equation
            if part.starts_with('{')
                && let Some((_, rest)) = Self::split_braces(part) {
                    out.push_str(rest);
                    continue;
                }
            if let Some(rest) = part.strip_prefix('i').or_else(|| part.strip_prefix("[]")) {
                out.push_str(rest);
            } else {
                out.push_str(part);
            }
        }
        out
    }
}

impl From<String> for Description {
    fn from(s: String) -> Description {
        Description{public: s, private: "".to_owned()}
    }
}

impl From<&str> for Description {
    fn from(s: &str) -> Description {
        Description{public: s.to_owned(), private: "".to_owned()}
    }
}


#[derive(Debug, Clone, Copy)]
pub enum DescIdx {
    None,
    Array(u16),
    RegBus(u16,u16),
    FieldBus(u16,u16),
}

impl DescIdx {
    pub fn array(idx: u16) -> Self {
        DescIdx::Array(idx)
    }

    pub fn reg_bus(width: u16, idx: u16) -> Self {
        DescIdx::RegBus(width, idx)
    }

    pub fn field_bus(width: u16, idx: u16) -> Self {
        DescIdx::FieldBus(width, idx)
    }

    pub fn val(&self) -> u16 {
        match self {
            DescIdx::None => 0,
            DescIdx::Array(i) => *i,
            DescIdx::RegBus(_, i) => *i,
            DescIdx::FieldBus(_, i) => *i,
        }
    }

    pub fn lsb(&self) -> u16 {
        match self {
            DescIdx::None => 0,
            DescIdx::Array(i) => *i,
            DescIdx::RegBus(w, i) => i*w,
            DescIdx::FieldBus(w, i) => i*w,
        }
    }

    pub fn msb(&self) -> u16 {
        match self {
            DescIdx::None => 0,
            DescIdx::Array(i) => *i,
            DescIdx::RegBus(w, i) => i * w + w - 1,
            DescIdx::FieldBus(w, i) => i * w + w - 1,
        }
    }

    pub fn width(&self) -> u16 {
        match self {
            DescIdx::None => 0,
            DescIdx::Array(_) => 0,
            DescIdx::RegBus(w, _) => *w,
            DescIdx::FieldBus(w, _) => *w,
        }
    }
}

impl From<u16> for DescIdx {
    fn from(value: u16) -> Self {
        DescIdx::Array(value)
    }
}

impl From<usize> for DescIdx {
    fn from(value: usize) -> Self {
        DescIdx::Array(value as u16)
    }
}