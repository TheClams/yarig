use crate::rifgen::{AddressKind, Context, RegInst};

use winnow::{
    ascii::space0, combinator::{alt, delimited, opt, preceded, terminated}, error::ErrorKind, token::take_until, Parser
};

use super::{identifier, parser_expr::{parse_expr, ExprTokens}, val_u64, val_u16, ws, Res, ResF};

//--------------------------------
// Page properties

pub fn page_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
    terminated(
        alt((
            ws("baseAddress").value(Context::BaseAddress),
            ws("addrWidth").value(Context::AddrWidth),
            ws("description").value(Context::Description),
            ws("desc").value(Context::Description),
            ws("clkEn").value(Context::HwClkEn),
            ws("external").value(Context::External),
            ws("optional").value(Context::Optional),
            ws("registers").value(Context::Registers),
            ws("instances").value(Context::Instances),
            ws("include").value(Context::Include),
        )),
        opt(ws(":")),
    ).parse_next(input)
}

//--------------------------------
// Instances properties

pub fn is_auto(input: &str) -> ResF<bool> {
    alt((
        ws("auto").value(true),
        space0.value(false)
    )).parse(input)
}

// - reg_name[[array_size]] [= regType] [(groupName)] [@ regAddr]
pub fn reg_inst(input: &str) -> ResF<RegInst> {
    (
        preceded(ws("-"), ws(identifier)),
        opt(
            delimited(
                ws("["),
                take_until(1..,"]"),
                ws("]")
            )
        ).try_map(|s| if let Some(expr) = s {parse_expr(expr)} else {Ok(ExprTokens::new(0))}),
        opt(preceded(ws("="), ws(identifier))),
        opt(delimited(ws("("), identifier, ws(")"))),
        opt((
            alt((
                ws("@+=").value(AddressKind::RelativeSet),
                ws("@+").value(AddressKind::Relative),
                ws("@").value(AddressKind::Absolute),
            )),
            ws(val_u64),
        )),
    ).parse(input)
    .map(|v| v.into())
}

pub fn reg_inst_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
    terminated(
        alt((
            alt((ws("description"),ws("desc"))).value(Context::Description),
            ws("parameters").value(Context::Parameters),
            ws("info").value(Context::Info),
            ws("optional").value(Context::Optional),
            ws("hidden").value(Context::Hidden),
            alt((ws("disabled"),ws("disable"))).value(Context::Disabled),
            ws("reserved").value(Context::Reserved),
            ws("hw").value(Context::HwAccess),
            delimited(ws("["), val_u16, ws("].")).try_map(|v| -> Result<Context, ErrorKind> {Ok(Context::RegIndex(v))}),
            terminated(identifier, ".").try_map(|v| -> Result<Context, ErrorKind> {Ok(Context::Item(v.into()))}),
            reg_inst_field_array,
        )),
        opt(alt((ws(":"),ws("=")))),
    ).parse_next(input)
}

pub fn reg_inst_array_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
    terminated(
        alt((
            alt((ws("description"),ws("desc"))).value(Context::Description),
            ws("optional").value(Context::Optional),
            ws("info").value(Context::Info),
            ws("hidden").value(Context::Hidden),
            ws("reserved").value(Context::Reserved),
            alt((ws("disabled"),ws("disable"))).value(Context::Disabled),
            ws("hw").value(Context::HwAccess),
            terminated(identifier, ".").try_map(|v| -> Result<Context, ErrorKind> {Ok(Context::Item(v.into()))}),
        )),
        opt(alt((ws(":"),ws("=")))),
    ).parse_next(input)
}

pub fn reg_inst_field_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
    terminated(
        alt((
            alt((ws("description"),ws("desc"))).value(Context::Description),
            ws("info").value(Context::Info),
            ws("optional").value(Context::Optional),
            ws("hidden").value(Context::Hidden),
            ws("reserved").value(Context::Reserved),
            alt((ws("disabled"),ws("disable"))).value(Context::Disabled),
            alt((ws("reset"),ws("rst"))).value(Context::HwReset),
            ws("limit").value(Context::Limit),
        )),
        opt(alt((ws(":"),ws("=")))),
    ).parse_next(input)
}

pub fn reg_inst_field_array<'a>(input: &mut &'a str) -> Res<'a, Context> {
    (
        identifier,
        delimited(ws("["), val_u16, ws("].")),
    ).try_map(|v| -> Result<Context, ErrorKind> {Ok(Context::FieldIndex((v.0.to_owned(),v.1)))})
    .parse_next(input)
}

//--------------------------------
// Tests

#[cfg(test)]
mod tests_parsing {

    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_page_properties() {
        assert_eq!(
            page_properties(&mut "baseAddress : 0x240"),
            Ok(Context::BaseAddress)
        );
        assert_eq!(page_properties(&mut "registers: "), Ok(Context::Registers));
        assert_eq!(
            page_properties(&mut "description: text with 9 and €"),
            Ok(Context::Description)
        );
        assert_eq!(
            page_properties(&mut "instances: auto "),
            Ok(Context::Instances)
        );
    }

    #[test]
    fn test_is_auto() {
        assert_eq!(is_auto("auto"), Ok(true));
        assert_eq!(is_auto("  "), Ok(false));
        assert_eq!(is_auto("anything else").is_err(),true);
    }

    // - reg_name[[array_size]] [= regType] [(groupName)] [@ regAddr]
    #[test]
    fn test_reg_inst() {
        assert_eq!(
            reg_inst("- reg_name[4] = reg_type (reg_group) @ 0x10 "),
            Ok(RegInst {
                inst_name: "reg_name".to_owned(),
                type_name: "reg_type".to_owned(),
                group_name: "reg_group".to_owned(),
                addr_kind: AddressKind::Absolute,
                addr: 0x10,
                array: parse_expr("4").expect("Parse 4 cannot fail"),
                reg_override: HashMap::new(),
            })
        );
        assert_eq!(
            reg_inst("- reg_name @ 0x04"),
            Ok(RegInst {
                inst_name: "reg_name".to_owned(),
                type_name: "reg_name".to_owned(),
                group_name: "".to_owned(),
                addr_kind: AddressKind::Absolute,
                addr: 0x04,
                array: ExprTokens::new(0),
                reg_override: HashMap::new(),
            })
        );
        assert_eq!(
            reg_inst_field_array(&mut "idx[0].reset = 0"),
            Ok(Context::FieldIndex(("idx".to_owned(),0)))
        );
    }
}
