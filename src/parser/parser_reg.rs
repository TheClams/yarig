use crate::rifgen::{
    Context, InterruptClr, InterruptInfo, InterruptPropTuple, InterruptTrigger, RegDef, ResetVal,
};

use winnow::{
    ascii::{space0, Caseless},
    combinator::{alt, delimited, opt, permutation, preceded, terminated},
    Parser
};

use super::{identifier, scoped_identifier, item_start, quoted_string, reset_val, val_u8_or_param, ws, Res, ResF};

// Register declaration format is the following
// - reg_name : (group_name) "register description"
// where the (group_name) and description are optionnal
fn reg_decl_l<'a>(input: &mut &'a str) -> Res<'a, RegDef> {
    let name = identifier(input)?;
    let array_size = opt(delimited("[", val_u8_or_param, "]")).parse_next(input)?;
    ws(":").parse_next(input)?;
    let group_name = opt(delimited("(", scoped_identifier, ")")).parse_next(input)?;
    let desc = opt(quoted_string).parse_next(input)?;
    Ok(
        RegDef::new(name, group_name, array_size, desc.unwrap_or_default()),
    )
}

pub fn reg_decl(input: &str) -> ResF<RegDef> {
    reg_decl_l.parse(input)
}

pub fn reg_incl_or_decl<'a>(input: &mut &'a str) -> Res<'a, Context> {
    alt((
        preceded(opt("-"), ws("include")).value(Context::Include),
        ws("-").value(Context::Registers),
    )).parse_next(input)
}

pub fn reg_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
    terminated(
        alt((
            alt((ws("description"),ws("desc"))).value(Context::Description),
            ws("enable.description").value(Context::DescIntrEnable),
            ws("mask.description").value(Context::DescIntrMask),
            ws("pending.description").value(Context::DescIntrPending),
            ws("clock").value(Context::HwClock),
            ws("hwReset").value(Context::HwReset),
            ws(Caseless("clkEn")).value(Context::HwClkEn),
            ws("clear").value(Context::HwClear),
            ws("externalDone").value(Context::ExternalDone),
            ws("external").value(Context::External),
            ws("interrupt").value(Context::Interrupt),
            ws("alt").value(Context::InterruptAlt),
            ws("hidden").value(Context::Hidden),
            alt((ws("disabled"),ws("disable"))).value(Context::Disabled),
            ws("reserved").value(Context::Reserved),
            ws("optional").value(Context::Optional),
            ws("info").value(Context::Info),
            ws("wrPulse").value(Context::RegPulseWr),
            ws("rdPulse").value(Context::RegPulseRd),
            ws("accPulse").value(Context::RegPulseAcc),
            terminated(identifier,".").map(|v| Context::PathStart(v.to_owned())),
        )),
        opt(alt((ws(":"), ws("="), space0))),
    )
    .parse_next(input)
}

pub fn intr_desc<'a>(input: &mut &'a str) -> Res<'a, Context> {
    terminated(
        alt((
            ws("enable.description").value(Context::DescIntrEnable),
            ws("mask.description").value(Context::DescIntrMask),
            ws("pending.description").value(Context::DescIntrPending),
            ws("enable.desc").value(Context::DescIntrEnable),
            ws("mask.desc").value(Context::DescIntrMask),
            ws("pending.desc").value(Context::DescIntrPending),
        )),
        opt(alt((ws(":"), ws("="), space0))),
    )
    .parse_next(input)
}

pub fn reg_properties_or_item<'a>(input: &mut &'a str) -> Res<'a, Context> {
    alt((
        reg_properties,
        item_start,
    )).parse_next(input)
}

// Interrupt properties are : high|low|rising|falling|edge [rclr|wclr|w0clr|w1clr|hwclr] [en[=valEnable]] [mask[=valMask]] [pending]`
pub fn reg_interrupt_trigger<'a>(input: &mut &'a str) -> Res<'a, InterruptTrigger> {
    alt((
        ws("high").value(InterruptTrigger::High),
        ws("low").value(InterruptTrigger::Low),
        ws("rising").value(InterruptTrigger::Rising),
        ws("falling").value(InterruptTrigger::Falling),
        ws("edge").value(InterruptTrigger::Edge),
    )).parse_next(input)
}

pub fn reg_interrupt_clr<'a>(input: &mut &'a str) -> Res<'a, InterruptClr> {
    alt((
        ws("rclr").value(InterruptClr::Read),
        ws("wclr").value(InterruptClr::Write1),
        ws("w1clr").value(InterruptClr::Write1),
        ws("w0clr").value(InterruptClr::Write0),
        ws("hwclr").value(InterruptClr::Hw),
        ws("hwclr").value(InterruptClr::Read),
    )).parse_next(input)
}

pub fn reg_interrupt_en<'a>(input: &mut &'a str) -> Res<'a, ResetVal> {
    alt(("enable", "en")).parse_next(input)?;
    preceded("=", reset_val).parse_next(input).or_else(|_| Ok(ResetVal::Unsigned(0)))
}

pub fn reg_interrupt_mask<'a>(input: &mut &'a str) -> Res<'a, ResetVal> {
    "mask".parse_next(input)?;
    preceded("=", reset_val).parse_next(input).or_else(|_| Ok(ResetVal::Unsigned(0)))
}

pub fn reg_interrupt_perm<'a>(input: &mut &'a str) -> Res<'a, InterruptPropTuple> {
    permutation((
        opt(ws(reg_interrupt_trigger)),
        opt(ws(reg_interrupt_clr)),
        opt(ws(reg_interrupt_en)),
        opt(ws(reg_interrupt_mask)),
        opt(ws("pending").value(true)),
    )).parse_next(input)
}

pub fn reg_interrupt<'a>(input: &mut &'a str, name: &str ) -> Res<'a, InterruptInfo> {
    let mut info = reg_interrupt_perm(input)?;
    let mut r_tmp;
    let mut cont;
    for _ in 0..4 {
        r_tmp = reg_interrupt_perm(input)?;
        cont = false;
        // Update main info structure for each none
        if r_tmp.0.is_some() {
            info.0 = r_tmp.0;
            cont = true;
        }
        if r_tmp.1.is_some() {
            info.1 = r_tmp.1;
            cont = true;
        }
        if r_tmp.2.is_some() {
            info.2 = r_tmp.2.clone();
            cont = true;
        }
        if r_tmp.3.is_some() {
            info.3 = r_tmp.3.clone();
            cont = true;
        }
        if r_tmp.4.is_some() {
            info.4 = r_tmp.4;
            cont = true;
        }
        if !cont {
            break;
        }
    }
    Ok(InterruptInfo::new(name,info))
}

pub fn reg_pulse_info<'a>(input: &mut &'a str, reg_clk: &str, init: bool) -> Res<'a, String> {
    let is_reg = alt((
        ws("reg").value(true),
        ws("comb").value(false),
        space0.value(init),
    ))
    .parse_next(input)?;
    if let Some(name) = opt(identifier).parse_next(input)? {
        Ok(name.to_owned())
    } else if is_reg {
        Ok(reg_clk.to_owned())
    } else {
        Ok("".to_string())
    }

}

#[cfg(test)]
mod tests_parsing {
    use super::*;
    use crate::rifgen::{InterruptDesc, ResetVal};

    #[test]
    fn test_interrupt() {
        assert_eq!(
            reg_interrupt(&mut "edge w1clr enable=0x1337 mask=0xCAFE pending", ""),
            Ok(
                InterruptInfo {
                    name: "".to_owned(),
                    trigger: InterruptTrigger::Edge,
                    clear: InterruptClr::Write1,
                    description: InterruptDesc::default(),
                    enable: Some(ResetVal::Unsigned(0x1337)),
                    mask: Some(ResetVal::Unsigned(0xCAFE)),
                    pending: true,
                }
            )
        );
        assert_eq!(
            reg_interrupt(&mut "high mask pending en=0xCAFE hwclr", "event"),
            Ok(
                InterruptInfo {
                    name: "event".to_owned(),
                    trigger: InterruptTrigger::High,
                    clear: InterruptClr::Hw,
                    description: InterruptDesc::default(),
                    enable: Some(ResetVal::Unsigned(0xCAFE)),
                    mask: Some(ResetVal::Unsigned(0)),
                    pending: true,
                }
            )
        );
        // Check default values
        assert_eq!(
            reg_interrupt(&mut "en", "intr"),
            Ok(
                InterruptInfo {
                    name: "intr".to_owned(),
                    trigger: InterruptTrigger::High,
                    clear: InterruptClr::Read,
                    description: InterruptDesc::default(),
                    enable: Some(ResetVal::Unsigned(0)),
                    mask: None,
                    pending: false,
                }
            )
        );
    }
}
