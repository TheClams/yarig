use crate::rifgen::{
    Context, InterruptClr, InterruptPropTuple, InterruptTrigger, RegDef, ResetValP,
};

use winnow::{
    ascii::{space0, Caseless}, combinator::{alt, delimited, opt, preceded, terminated, unordered_seq}, error::StrContext, Parser
};

use super::{identifier, scoped_identifier, item_start, quoted_string, reset_val, val_u8_or_param, ws, Res, ResF};

// Register declaration format is the following
// - reg_name : (group_name) "register description"
// where the (group_name) and description are optional
fn reg_decl_l<'a>(input: &mut &'a str) -> Res<'a, RegDef> {
    let name = identifier(input)?;
    let array_size = opt(delimited("[", val_u8_or_param, "]")).parse_next(input)?;
    ws(":").parse_next(input)?;
    let group_name = opt(delimited("(", scoped_identifier, ")")).parse_next(input)?;
    let desc = opt(quoted_string).parse_next(input)?;
    let mut r = RegDef::new(name, group_name, array_size, desc.unwrap_or_default());
    r.src.has_inline_desc = desc.is_some();
    Ok(r)
}

pub fn reg_decl(input: &str) -> ResF<'_, RegDef> {
    reg_decl_l.parse(input)
}

pub fn reg_incl_or_decl<'a>(input: &mut &'a str) -> Res<'a, Context> {
    alt((
        preceded(opt("-"), ws("include")).value(Context::Include),
        ws("-").value(Context::Registers),
    )).context(StrContext::Label("register declaration"))
    .parse_next(input)
}

pub fn reg_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
    terminated(
        alt((
            alt((
                alt((ws("description"),ws("desc"))).value(Context::Description),
                ws("enable.description").value(Context::DescIntrEnable),
                ws("mask.description").value(Context::DescIntrMask),
                ws("pending.description").value(Context::DescIntrPending),
                ws("info").value(Context::Info),
                ws("wrPulse").value(Context::RegPulseWr),
                ws("rdPulse").value(Context::RegPulseRd),
                ws("accPulse").value(Context::RegPulseAcc),
            )),
            alt((
                ws("clock").value(Context::HwClock),
                ws("hwReset").value(Context::HwReset),
                ws(Caseless("clkEn")).value(Context::HwClkEn),
                ws("clear").value(Context::HwClear),
                ws("hidden").value(Context::Hidden),
                alt((ws("disabled"),ws("disable"))).value(Context::Disabled),
                ws("reserved").value(Context::Reserved),
                ws("optional").value(Context::Optional),
            )),
            alt((
                ws("externalDone").value(Context::ExternalDone),
                ws("external").value(Context::External),
                ws("interrupt").value(Context::Interrupt),
                ws("alt").value(Context::InterruptAlt),
                terminated(identifier,".").map(|v| Context::PathStart(v.to_owned())),
            )),
        )),
        opt(alt((ws(":"), ws("="), space0))),
    )
    .context(StrContext::Label("register property"))
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
    .context(StrContext::Label("interrupt description"))
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
    ))
    .context(StrContext::Label("register interrupt trigger"))
    .parse_next(input)
}

pub fn reg_interrupt_clr<'a>(input: &mut &'a str) -> Res<'a, InterruptClr> {
    alt((
        ws("rclr").value(InterruptClr::Read),
        ws("wclr").value(InterruptClr::Write1),
        ws("w1clr").value(InterruptClr::Write1),
        ws("w0clr").value(InterruptClr::Write0),
        ws("hwclr").value(InterruptClr::Hw),
        ws("hwclr").value(InterruptClr::Read),
    ))
    .context(StrContext::Label("register interrupt clear"))
    .parse_next(input)
}

pub fn reg_interrupt_en<'a>(input: &mut &'a str) -> Res<'a, ResetValP> {
    alt(("enable", "en"))
        .context(StrContext::Label("register interrupt enable"))
        .parse_next(input)?;
    preceded("=", reset_val)
        .parse_next(input).or_else(|_| Ok(ResetValP::Unsigned(0)))
}

pub fn reg_interrupt_mask<'a>(input: &mut &'a str) -> Res<'a, ResetValP> {
    "mask".context(StrContext::Label("register interrupt mask")).parse_next(input)?;
    preceded("=", reset_val).parse_next(input).or_else(|_| Ok(ResetValP::Unsigned(0)))
}

pub fn reg_interrupt_perm<'a>(input: &mut &'a str) -> Res<'a, InterruptPropTuple> {
    unordered_seq!((
        opt(ws(reg_interrupt_trigger)),
        opt(ws(reg_interrupt_clr)),
        opt(ws(reg_interrupt_en)),
        opt(ws(reg_interrupt_mask)),
        opt(ws("pending").value(true)),
    ))
    .context(StrContext::Label("register interrupt property"))
    .parse_next(input)
}

pub fn reg_interrupt<'a>(input: &mut &'a str) -> Res<'a, InterruptPropTuple> {
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
    Ok(info)
    // Ok(InterruptInfo::new(name,info))
}

pub fn reg_pulse_info<'a>(input: &mut &'a str, reg_clk: &str, init: bool) -> Res<'a, String> {
    let is_reg = alt((
        ws("reg").value(true),
        ws("comb").value(false),
        space0.value(init),
    ))
    .context(StrContext::Label("register pulse (reg/comb)"))
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
    use crate::rifgen::{InterruptDesc, InterruptInfo, ResetValP};

    #[test]
    fn test_interrupt() {
        assert_eq!(
            InterruptInfo::new("",reg_interrupt(&mut "edge w1clr enable=0x1337 mask=0xCAFE pending").unwrap()),
            InterruptInfo {
                name: "".to_owned(),
                trigger: InterruptTrigger::Edge,
                clear: InterruptClr::Write1,
                description: InterruptDesc::default(),
                enable: Some(ResetValP::Unsigned(0x1337)),
                mask: Some(ResetValP::Unsigned(0xCAFE)),
                pending: true,
            }
        );
        assert_eq!(
            InterruptInfo::new("event",reg_interrupt(&mut "high mask pending en=0xCAFE hwclr").unwrap()),
            InterruptInfo {
                name: "event".to_owned(),
                trigger: InterruptTrigger::High,
                clear: InterruptClr::Hw,
                description: InterruptDesc::default(),
                enable: Some(ResetValP::Unsigned(0xCAFE)),
                mask: Some(ResetValP::Unsigned(0)),
                pending: true,
            }
        );
        // Check default values
        assert_eq!(
            InterruptInfo::new("intr",reg_interrupt(&mut "en").unwrap()),
            InterruptInfo {
                name: "intr".to_owned(),
                trigger: InterruptTrigger::High,
                clear: InterruptClr::Read,
                description: InterruptDesc::default(),
                enable: Some(ResetValP::Unsigned(0)),
                mask: None,
                pending: false,
            }
        );
    }
}
