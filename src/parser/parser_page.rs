use crate::rifgen::{AddressKind, Context, InstMode, RegInst};

use winnow::{
    ascii::space0,
    combinator::{alt, delimited, opt, preceded, separated, separated_pair, terminated},
    error::StrContext,
    token::take_until,
    Parser
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
    ).context(StrContext::Label("page property"))
    .parse_next(input)
}

//--------------------------------
// Instances properties

pub fn is_auto(input: &str) -> ResF<InstMode> {
    alt((
        ws("auto-legacy").value(InstMode::AutoLegacy),
        ws("auto").value(InstMode::Automatic),
        space0.value(InstMode::Manual)
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
    ).context(StrContext::Label("register instance"))
    .parse(input)
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
            terminated(index_list,".").map(Context::RegIndex),
            terminated(identifier, ".").map(|v| Context::Item(v.into())),
            terminated(reg_inst_field_array, "."),
        )),
        opt(alt((ws(":"),ws("=")))),
    ).context(StrContext::Label("register instance property"))
    .parse_next(input)
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
            terminated(identifier, ".").map(|v| Context::Item(v.into())),
        )),
        opt(alt((ws(":"),ws("=")))),
    ).context(StrContext::Label("register instance array property"))
    .parse_next(input)
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
    ).context(StrContext::Label("register field property"))
    .parse_next(input)
}

pub fn reg_inst_field_array<'a>(input: &mut &'a str) -> Res<'a, Context> {
    (identifier,index_list)
    .map(|v| Context::FieldIndex((v.0.to_owned(),v.1)))
    .context(StrContext::Label("register field array"))
    .parse_next(input)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InstIndexElement {Value(u16), Range(u16,u16)}

pub fn inst_index_range<'a>(input: &mut &'a str) -> Res<'a, InstIndexElement > {
    separated_pair(val_u16, ws(alt((":","-",".."))), val_u16)
    .map(|(low,high)| InstIndexElement::Range(low,high))
    .parse_next(input)
}

pub fn inst_index_val<'a>(input: &mut &'a str) -> Res<'a, InstIndexElement > {
    val_u16
    .map(InstIndexElement::Value)
    .parse_next(input)
}

pub fn index_list<'a>(input: &mut &'a str) -> Res<'a, Vec<u16>> {
    delimited(
        ws("["),
        separated(1.., alt((inst_index_range,inst_index_val)), ws(",")),
        ws("]")
    ).map(|list : Vec<InstIndexElement>| {
        let mut vec : Vec<u16> = Vec::with_capacity(list.len());
        for e in list.iter() {
            match e {
                InstIndexElement::Value(v) => vec.push(*v),
                InstIndexElement::Range(l,h) => vec.extend(*l..*h+1),
            }
        }
        vec
    }).parse_next(input)
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
        assert_eq!(is_auto("auto"), Ok(InstMode::Automatic));
        assert_eq!(is_auto("auto-legacy"), Ok(InstMode::AutoLegacy));
        assert_eq!(is_auto("  "), Ok(InstMode::Manual));
        assert!(is_auto("anything else").is_err());
    }

    #[test]
    fn test_index_list() {
        assert_eq!(index_list(&mut "[1:3]"), Ok(vec![1,2,3]));
        assert_eq!(index_list(&mut "[1-3]"), Ok(vec![1,2,3]));
        assert_eq!(index_list(&mut "[1..3]"), Ok(vec![1,2,3]));
        assert_eq!(index_list(&mut "[3]"), Ok(vec![3]));
        assert_eq!(index_list(&mut "[3,7]"), Ok(vec![3,7]));
        assert_eq!(index_list(&mut "[3:5,7,9:11]"), Ok(vec![3,4,5,7,9,10,11]));
        assert!(index_list(&mut "[3:5,]").is_err());
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
            reg_inst_properties(&mut "[0].description"),
            Ok(Context::RegIndex(vec![0]))
        );
        assert_eq!(
            reg_inst_properties(&mut "[0,3].description"),
            Ok(Context::RegIndex(vec![0,3]))
        );
        assert_eq!(
            reg_inst_field_array(&mut "idx[0].reset = 0"),
            Ok(Context::FieldIndex(("idx".to_owned(),vec![0])))
        );
        assert_eq!(
            reg_inst_field_array(&mut "idx[0,4:6].reset = 0"),
            Ok(Context::FieldIndex(("idx".to_owned(),vec![0,4,5,6])))
        );
    }
}
