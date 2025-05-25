use crate::{error::RifError, rifgen::{Context, LogicExpr, Width}};

use winnow::{
    ascii::{alpha1, alphanumeric1, digit0, digit1, hex_digit1, multispace0, space0, Caseless},
    combinator::{alt, delimited, eof, opt, preceded, repeat, repeat_till, separated_pair, terminated},
    error::{self, ContextError, ErrMode, ParseError, StrContext},
    stream::{AsChar, Stream, StreamIsPartial},
    token::{any, take_until}, ModalParser, ModalResult, Parser
};

use super::parser_logic_expr::parse_logic_expr;

//--------------------------------
// General parsing rules
pub type Res<'a, T> = ModalResult<T>;
pub type ResF<'a, T> = Result<T, ParseError<&'a str, ContextError>>;

//
pub fn ws<I, O, E: error::ParserError<I>, F>(inner: F) -> impl ModalParser<I, O, E>
where
    I: StreamIsPartial + Stream,
    <I as Stream>::Token: AsChar + Copy,
    F: ModalParser<I, O, E>,
{
    delimited(multispace0, inner, multispace0)
}

pub fn identifier<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    (
        alt((alpha1, "_")),
        repeat::<_, _, Vec<&str>, _, _>(0.., alt((alphanumeric1, "_"))),
    )
        .take()
        .context(StrContext::Label("identifier"))
        .parse_next(input)
}

pub fn identifier_last(input: &str) -> ResF<&str> {
    ws(identifier).parse(input)
}

pub fn scoped_identifier<'a>(input: &mut &'a str) -> Res<'a,(Option<&'a str>,&'a str)> {
    (
        opt(terminated(identifier,"::")),
        identifier
    ).context(StrContext::Label("scoped identifier")).parse_next(input)
}

// TODO: find a way to ensure the identifier is not followed by a non space character
// check : https://stackoverflow.com/questions/74159691/parse-eof-or-a-character-in-winnow
pub fn signal_name<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    alt((
        preceded(".", identifier).take(),
        (identifier, opt(preceded(".", identifier))).take()
    )).context(StrContext::Label("signal name")).parse_next(input)
}

pub fn path_name<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    (identifier, repeat::<_, _, (), _, _>(0..,preceded(".", identifier)))
        .take()
        .context(StrContext::Label("path name"))
        .parse_next(input)
}

#[allow(dead_code)]
pub fn signal_name_last(input: &str) -> ResF<&str> {
    signal_name.parse(input)
}

pub fn logic_expr<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    (ws("("), take_until_unbalanced('(', ')'), ")")
        .take()
        .context(StrContext::Label("logic expression"))
        .parse_next(input)
}

pub fn take_until_unbalanced<'a>(
    opening_bracket: char,
    closing_bracket: char,
) -> impl Fn(&mut &'a str) -> Res<'a, &'a str> {
    move |i: &mut &'a str| {
        let mut index = 0;
        let mut bracket_counter = 0;
        while let Some(n) = &i[index..].find(&[opening_bracket, closing_bracket, '\\'][..]) {
            index += n;
            let mut it = i[index..].chars();
            match it.next().unwrap_or_default() {
                c if c == opening_bracket => {
                    bracket_counter += 1;
                    index += opening_bracket.len_utf8();
                }
                c if c == closing_bracket => {
                    // Closing bracket.
                    bracket_counter -= 1;
                    index += closing_bracket.len_utf8();
                }
                // Can not happen.
                _ => unreachable!(),
            };
            // We found the unmatched closing bracket.
            if bracket_counter == -1 {
                // We do not consume it.
                index -= closing_bracket.len_utf8();
                return Ok(i.next_slice(index));
            };
        }
        if bracket_counter == 0 {
            println!("[take_until_unbalanced] Found 0 : {}", i);
            Ok(i.next_slice(i.len()))
        } else {
            Err(ErrMode::Backtrack(ContextError::new()))
        }
    }
}

pub fn signal_or_expr(input: &str) -> Result<LogicExpr,RifError> {
    let s = alt((ws(signal_name), ws(logic_expr))).parse(input)?;
    parse_logic_expr(s)
}

pub fn opt_signal_or_expr(input: &str) -> Result<Option<LogicExpr>,RifError> {
    let s = alt((ws(signal_name), ws(logic_expr), multispace0)).parse(input)?;
    if s.trim().is_empty() {
        Ok(None)
    } else {
        let e = parse_logic_expr(s)?;
        Ok(Some(e))
    }
}

// Return the number of space or tabs
// enum IndentType {Unknown, Spaces, Tabs}
pub fn indentation<'a>(input: &mut &'a str) -> Res<'a,usize> {
    let indent = multispace0(input)?;
    if indent.contains(' ') && indent.contains('\t') {
        Err(ErrMode::Cut(ContextError::new()))
    } else {
        Ok(indent.len())
    }
}

pub fn parse_bool<'a>(input: &mut &'a str) -> Res<'a, bool> {
    alt((
        ws(Caseless("true")).value(true),
        ws(Caseless("false")).value(false),
        ws("1").value(true),
        ws("0").value(false),
    ))
    .context(StrContext::Label("boolean"))
    .parse_next(input)
}

pub fn bool_or_default(input: &str, def: bool) -> ResF<bool> {
    alt((
        parse_bool,
        space0.value(def),
    )).parse(input)
}

pub fn quoted_string<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    delimited(ws('\"'), take_until(0..,'"'), ws('"'))
        .context(StrContext::Label("quoted string"))
        .parse_next(input)
}

pub fn unquoted_string<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    ws( repeat_till::<_, _, Vec<char>, _, _, _, _>(0..,any,eof).take())
        .context(StrContext::Label("unquoted string"))
        .parse_next(input)
}

pub fn desc(input: &str) -> ResF<&str> {
    alt((quoted_string, unquoted_string))
        .context(StrContext::Label("description"))
        .parse(input)
}

pub fn is_hidden<'a>(input: &mut &'a str) -> Res<'a, bool> {
    alt((
        terminated(".hidden", opt(ws(":"))).value(true),
        space0.value(false),
    )).parse_next(input)
}

/// parse a comment starting by // or # or just spaces
pub fn comment(input: &str) -> ResF<()> {
    alt((
       (alt((ws("//"), ws("#"))), repeat_till::<_, _, Vec<char>, _, _, _, _>(0..,any,eof)).take(),
        space0,
    ))
    .value(())
    .parse(input)
}

pub fn item<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    preceded("-", terminated(ws(identifier), opt(ws(":"))))
        .context(StrContext::Label("item"))
        .parse_next(input)
}

pub fn vec_id(input: &str) -> ResF<Vec<&str>> {
    repeat(1.., ws(identifier)).parse(input)
}

pub fn item_cntxt<'a>(input: &mut &'a str) -> Res<'a, Context> {
    item(input).map(|id| Context::Item(id.to_owned()))
}

pub fn item_start<'a>(input: &mut &'a str) -> Res<'a, Context> {
    ws("-")
        .value(Context::Item("".to_owned()))
        .context(StrContext::Label("item start (-)"))
        .parse_next(input)
}

pub fn key_val(input: &str) -> ResF<(&str, &str)> {
    preceded(
        "-",
        separated_pair(ws(identifier), opt(alt(("=", ":"))), unquoted_string),
    )
    .context(StrContext::Label("key/value pair"))
    .parse(input)
}

pub fn path_val(input: &str) -> ResF<(&str, &str)> {
    preceded(
        "-",
        separated_pair(ws(path_name), opt(alt(("=", ":"))), unquoted_string),
    )
    .context(StrContext::Label("path value"))
    .parse(input)
}

#[allow(clippy::from_str_radix_10)]
pub fn val_u8<'a>(input: &mut &'a str) -> Res<'a, u8> {
    alt((
        preceded((digit0, "'b"), digit1).try_map(|v| u8::from_str_radix(v, 2)),
        preceded((digit0, "'o"), digit1).try_map(|v| u8::from_str_radix(v, 8)),
        preceded((digit0, "'d"), digit1).try_map(|v| u8::from_str_radix(v, 10)),
        preceded((digit0, "'h"), hex_digit1).try_map(|v| u8::from_str_radix(v, 16)),
        preceded("0x", hex_digit1).try_map(|v| u8::from_str_radix(v, 16)),
        digit1.try_map(str::parse),
    ))
    .context(StrContext::Label("unsigned 8b"))
    .parse_next(input)
}

#[allow(clippy::from_str_radix_10)]
pub fn val_u16<'a>(input: &mut &'a str) -> Res<'a, u16> {
    alt((
        preceded((digit0, "'b"), digit1).try_map(|v| u16::from_str_radix(v, 2)),
        preceded((digit0, "'o"), digit1).try_map(|v| u16::from_str_radix(v, 8)),
        preceded((digit0, "'d"), digit1).try_map(|v| u16::from_str_radix(v, 10)),
        preceded((digit0, "'h"), hex_digit1).try_map(|v| u16::from_str_radix(v, 16)),
        preceded("0x", hex_digit1).try_map(|v| u16::from_str_radix(v, 16)),
        digit1.try_map(str::parse),
    ))
    .context(StrContext::Label("unsigned 16b"))
    .parse_next(input)
}

#[allow(dead_code)]
pub fn param<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    preceded("$", identifier).parse_next(input)
}

#[allow(dead_code)]
pub fn val_u8_or_param<'a>(input: &mut &'a str) -> Res<'a, Width> {
    alt((
        val_u8.map(Width::Value),
        param.map(|v| Width::Param(v.to_owned())),
    ))
    .context(StrContext::Label("unsigned 8b or parameter"))
    .parse_next(input)
}

pub fn val_u64<'a>(input: &mut &'a str) -> Res<'a, u64> {
    alt((
        preceded("0x", hex_digit1).try_map(|v| u64::from_str_radix(v, 16)),
        digit1.try_map(str::parse),
    ))
    .context(StrContext::Label("unsigned 64b"))
    .parse_next(input)
}

#[allow(clippy::from_str_radix_10)]
pub fn val_u128<'a>(input: &mut &'a str) -> Res<'a, u128> {
    alt((
        preceded((digit0, "'b"), digit1).try_map(|v| u128::from_str_radix(v, 2)),
        preceded((digit0, "'o"), digit1).try_map(|v| u128::from_str_radix(v, 8)),
        preceded((digit0, "'d"), digit1).try_map(|v| u128::from_str_radix(v, 10)),
        preceded((digit0, "'h"), hex_digit1).try_map(|v| u128::from_str_radix(v, 16)),
        preceded("0x", hex_digit1).try_map(|v| u128::from_str_radix(v, 16)),
        digit1.try_map(str::parse),
    ))
    .context(StrContext::Label("unsigned 128b"))
    .parse_next(input)
}

#[allow(clippy::from_str_radix_10)]
pub fn val_i128<'a>(input: &mut &'a str) -> Res<'a, i128> {
    alt((
        preceded((digit0, "'b"), digit1).try_map(|v| i128::from_str_radix(v, 2)),
        preceded((digit0, "'o"), digit1).try_map(|v| i128::from_str_radix(v, 8)),
        preceded((digit0, "'d"), digit1).try_map(|v| i128::from_str_radix(v, 10)),
        preceded((digit0, "'h"), hex_digit1).try_map(|v| i128::from_str_radix(v, 16)),
        preceded("0x", hex_digit1).try_map(|v| i128::from_str_radix(v, 16)),
        (opt(alt(("+", "-"))), digit1)
            .take()
            .try_map(|v| i128::from_str_radix(v, 10)),
    ))
    .context(StrContext::Label("signed 128b"))
    .parse_next(input)
}

#[allow(clippy::from_str_radix_10)]
pub fn val_isize<'a>(input: &mut &'a str) -> Res<'a, isize> {
    alt((
        preceded((digit0, "'b"), digit1).try_map(|v| isize::from_str_radix(v, 2)),
        preceded((digit0, "'o"), digit1).try_map(|v| isize::from_str_radix(v, 8)),
        preceded((digit0, "'d"), digit1).try_map(|v| isize::from_str_radix(v, 10)),
        preceded((digit0, "'h"), hex_digit1).try_map(|v| isize::from_str_radix(v, 16)),
        preceded("0x", hex_digit1).try_map(|v| isize::from_str_radix(v, 16)),
        (opt(alt(("+", "-"))), digit1)
            .take()
            .try_map(|v| isize::from_str_radix(v, 10)),
    ))
    .context(StrContext::Label("signed 128b"))
    .parse_next(input)
}

pub fn val_f64<'a>(i: &mut &'a str) -> Res<'a, f64> {
    winnow::ascii::float.context(StrContext::Label("floating value")).parse_next(i)
}

#[cfg(test)]
mod tests_parsing {

    use super::*;
    // use crate::rifgen::{EnumKind,FieldHwKind,FieldSwKind, ResetVal, Visibility};

    #[test]
    fn test_comment() {
        assert_eq!(comment(&mut "# comment #"), Ok(()));
        assert_eq!(comment(&mut "  // comment //"), Ok(()));
        assert_eq!(comment(&mut "  / not a comment").is_err(), true);
    }

    #[test]
    fn test_identifier() {
        assert_eq!(identifier(&mut "signal123"), Ok("signal123"));
        assert_eq!(identifier(&mut "_signal123"), Ok("_signal123"));
        assert_eq!(identifier(&mut "sig.field"), Ok("sig"));
        assert_eq!(identifier(&mut "0sig").is_err(), true);
        assert_eq!(identifier(&mut "+").is_err(), true);
    }

    #[test]
    fn test_signal() {
        assert_eq!(signal_name(&mut "signal123"), Ok("signal123"));
        assert_eq!(signal_name(&mut "sig.field"), Ok("sig.field"));
        assert_eq!(path_name(&mut "sig.field"), Ok("sig.field"));
        assert_eq!(path_name(&mut "l1.l2.l3."), Ok("l1.l2.l3"));
    }

    #[test]
    fn test_bool_or_default() {
        assert_eq!(parse_bool(&mut "true ??"), Ok(true));
        assert_eq!(parse_bool(&mut "True!"), Ok(true));
        assert_eq!(bool_or_default(&mut "  ", false), Ok(false));
        assert_eq!(bool_or_default(&mut "", true), Ok(true));
        assert_eq!(bool_or_default(&mut "0", true), Ok(false));
        assert_eq!(bool_or_default(&mut "1", false), Ok(true));
        assert_eq!(bool_or_default(&mut "true", false), Ok(true));
        assert_eq!(bool_or_default(&mut "True", false), Ok(true));
        assert_eq!(bool_or_default(&mut "True ? no !", false).is_err(), true);
        assert_eq!(bool_or_default(&mut "False", true), Ok(false));
        assert_eq!(bool_or_default(&mut "error", false).is_err(), true);
    }

    #[test]
    fn test_quoted_string() {
        assert_eq!(
            quoted_string(&mut r#""Simple quoted string" with following text"#),
            Ok("Simple quoted string")
        );
        assert_eq!(quoted_string(&mut "No quotes").is_err(), true);
        assert_eq!(quoted_string(&mut "\"No end quote").is_err(), true);
    }

    #[test]
    fn test_indent() {
        assert_eq!(indentation(&mut "No indent"), Ok(0));
        assert_eq!(indentation(&mut "  spaces: "), Ok(2));
        assert_eq!(indentation(&mut "	tab: "), Ok(1));
        assert_eq!(indentation(&mut "  	Tab & space").is_err(), true);
    }

    #[test]
    fn test_reset_val() {
        assert_eq!(val_u8(&mut "34 "), Ok(34));
        assert_eq!(val_u8(&mut "0x34 "), Ok(0x34));
        assert_eq!(val_u8(&mut "2'b10 "), Ok(2));
        assert_eq!(val_u8(&mut "6'o10 "), Ok(8));
        assert_eq!(val_u8(&mut "8'd10 "), Ok(10));
        assert_eq!(val_u8(&mut "8'h1A "), Ok(26));
    }

    #[test]
    fn test_key_val() {
        assert_eq!(key_val("- Key0 = 5"), Ok(("Key0", "5")));
        assert_eq!(key_val("- Key1 : 5*8 + 3"), Ok(("Key1", "5*8 + 3")));
        assert_eq!(key_val("- Key2 : log2($Key1)"), Ok(("Key2", "log2($Key1)")));
    }

    #[test]
    fn test_is_hidden() {
        assert_eq!(is_hidden(&mut "Description"), Ok(false));
        assert_eq!(is_hidden(&mut ".hidden: Description"), Ok(true));
        assert_eq!(is_hidden(&mut ".hidden : Description"), Ok(true));
        assert_eq!(is_hidden(&mut ".hidden"), Ok(true));
        assert_eq!(is_hidden(&mut ".hidden :"), Ok(true));
    }

    #[test]
    fn test_logic_expr() {
        assert_eq!(logic_expr(&mut "(3+5)" ), Ok("(3+5)"));
        assert_eq!(
            logic_expr(&mut "(s0 & (s1 | ~s2) & s3)"),
            Ok("(s0 & (s1 | ~s2) & s3)")
        );
        assert_eq!(logic_expr(&mut "(s1 & (s2)").is_err(), true);
    }
}
