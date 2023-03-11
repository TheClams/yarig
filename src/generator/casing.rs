#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Casing {#[default]
    /// Display string as-is
    Raw,
    /// lowercase with word separated by underscore
    Snake,
    /// All words starts with uppercase
    Pascal,
    /// All words starts with uppercase except first one
    Camel,
    /// lowercase with word separated by dash
    Kebab,
    /// All words starts with uppercase and are space-separated
    Title,
}

impl Casing {
    #[allow(dead_code)]
    pub fn format(&self, s: &str) -> String {
        let mut out = String::with_capacity(s.len()+8);
        let mut str_start = true;
        let mut word_start = true;
        let mut prev_start = false;
        let list_sep = ['_', '-', ' '];
        let sep = match self {
            Casing::Snake => Some('_'),
            Casing::Kebab => Some('-'),
            Casing::Title => Some(' '),
            _ => None,
        };
        for c in s.chars() {
            // Detect Word separation
            if !word_start {
                let is_sep = list_sep.contains(&c);
                if is_sep {
                    prev_start = false;
                }
                word_start = is_sep || c.is_uppercase();
                // Skip to next character if current is a word separator
                if word_start && self!=&Casing::Raw && is_sep {
                    continue;
                }
            }
            //
            if word_start & !prev_start {
                // Insert Word separator
                if !str_start {
                    if let Some(sep) = sep {
                        out.push(sep);
                    }
                }
                // Change casing of word start
                match self {
                    Casing::Pascal |
                    Casing::Title => out.push(c.to_ascii_uppercase()),
                    Casing::Snake |
                    Casing::Kebab => out.push(c.to_ascii_lowercase()),
                    // Camel : Capitalize first letter of each word except on first word
                    Casing::Camel if !str_start => out.push(c.to_ascii_uppercase()),
                    Casing::Camel => out.push(c.to_ascii_lowercase()),
                    // Raw don't touch
                    Casing::Raw => out.push(c),
                }
            } else {
                match self {
                    Casing::Raw => out.push(c),
                    _ => out.push(c.to_ascii_lowercase()),
                }
            }
            prev_start = word_start;
            word_start = false;
            str_start = false;
        }
        out
    }
}

pub trait ToCasing {
    fn to_casing(&self, casing: Casing) -> String;
}

impl<T> ToCasing for T where T: AsRef<str> {
    fn to_casing(&self, casing: Casing) -> String {
        casing.format(self.as_ref())
    }
}


#[cfg(test)]
mod tests_parsing {
    use super::*;

    #[test]
    fn test_casing() {
        let s = "value-with_DIFFERENT separatorCharacter";
        assert_eq!(
            Casing::Raw.format(s),
            s.to_owned()
        );
        assert_eq!(
            Casing::Snake.format(s),
            "value_with_different_separator_character".to_owned()
        );
        assert_eq!(
            Casing::Pascal.format(s),
            "ValueWithDifferentSeparatorCharacter".to_owned()
        );
        assert_eq!(
            Casing::Camel.format(s),
            "valueWithDifferentSeparatorCharacter".to_owned()
        );
        assert_eq!(
            Casing::Kebab.format(s),
            "value-with-different-separator-character".to_owned()
        );
        assert_eq!(
            Casing::Title.format(s),
            "Value With Different Separator Character".to_owned()
        );
    }
}