use crate::parser::{logic_expr, parser_expr::{parse_expr, ParamValues}};

/// Thin wrapper around string to ease update and get short/long descripion
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Description(String);

impl Description {
    pub fn updt(&mut self, desc: &str) {
        if !self.0.is_empty() {
            self.0.push('\n');
        }
        let mut line_it = desc.split("\\n").peekable();
        while let Some(line) = line_it.next() {
            self.0.push_str(line);
            if line_it.peek().is_some() {
                self.0.push('\n');
            }
        }
    }

    pub fn get(&self) -> &str {
        &self.0
    }

    pub fn get_short(&self) -> &str {
        if let Some(end) = self.0.find('\n') {
            &self.0[..end]
        } else {
            &self.0
        }
    }

    pub fn get_split(&self) -> (&str, Option<&str>) {
    	match self.0.find('\n') {
    		Some(mid) if mid < self.0.len()-1 => (&self.0[..mid], Some(&self.0[mid+1..self.0.len()])),
    		Some(end)  => (&self.0[..end], None),
    		None => (&self.0, None),
    	}
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn interpolate(&self, idx: u16) -> Description {
        // if self.0.starts_with("Gain and") {println!("{}",self.0)};
        let mut desc = String::with_capacity(self.0.len());
        let params = ParamValues::new_with_idx(idx as isize);
        let mut parts = self.0.split('$');
        desc.push_str(parts.next().unwrap_or(""));
        while let Some(mut s) = parts.next() {
            // Variable $i replaced by index
            if let Some(stripped) = s.strip_prefix('i') {
                desc.push_str(&format!("{idx}"));
                desc.push_str(stripped);
            }
            // Start of an equation
            else if s.starts_with('(') {
                let expr_s = logic_expr(&mut s).unwrap();
                let expr = parse_expr(expr_s).unwrap();
                let val = expr.eval(&params).unwrap();
                desc.push_str(&format!("{val}"));
                desc.push_str(s);
            } else {
                desc.push_str(s);
            }
        }
        Description(desc)
    }

    // Create a description with $f replace by a format string (for example u8.0 or s5.3)
    pub fn with_format(&self, format: &str) -> Description {
        let desc = self.0.replace("$f", format);
        Description(desc)
    }

    // Remove the $ special character from description:
    //  used when we do not want the interpolated version (for register base description typically)
    pub fn no_dollar(&self) -> Description {
        let desc = self.0.replace('$', "");
        Description(desc)
    }
}

impl From<String> for Description {
    fn from(d: String) -> Description {
        Description(d)
    }
}

impl From<&str> for Description {
    fn from(v: &str) -> Description {
        Description(v.to_owned())
    }
}
