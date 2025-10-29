use crate::{parser::comment, rifgen::{
    Access, ClkEn, Context, CounterInfo, CounterKind, EnumEntry, Field, FieldPos, FieldSwKind, InterruptInfoField, LimitP, LimitValueP, PasswordInfo, ResetValP
}};

use winnow::{
    ascii::{multispace0, space0, Caseless},
    combinator::{alt, delimited, opt, permutation, preceded, repeat_till, separated, separated_pair, terminated},
    error::StrContext,
    Parser
};

use super::{
    identifier, param, quoted_string, reg_interrupt_clr, reg_interrupt_trigger, signal_name, val_f64, val_i128, val_u128, val_u8, val_u8_or_param, ws, Res, ResF
};

/// Parse a reset value: this can be an integer, a float, the name of an enum or a parameter (string starting with $)
pub fn reset_val<'a>(input: &mut &'a str) -> Res<'a, ResetValP> {
    if input.starts_with('$') {
        param.map(|v| ResetValP::Param(v.to_owned())).parse_next(input)
    } else if input.starts_with(char::is_alphabetic) {
        identifier.map(|v| ResetValP::Enum(v.to_owned())).parse_next(input)
    } else {
        let is_float = input.split_whitespace().next().map(|r|
                r.contains(['e','E', '.']) && !r.contains(['\'', 'x', 'X'])
            ).unwrap_or(false);
        let is_signed = input.starts_with('-') || input.starts_with('+');
        match (is_float, is_signed) {
            (true, true)  => val_f64.map(ResetValP::FloatS).parse_next(input),
            (true, false) => val_f64.map(ResetValP::FloatU).parse_next(input),
            (false, true) => val_i128.map(ResetValP::Signed).parse_next(input),
            (false, false) => val_u128.map(ResetValP::Unsigned).parse_next(input),
        }
    }
}

/// Parse list of reset value contained between curly bracket and comma separated
pub fn reset_val_arr<'a>(input: &mut &'a str) -> Res<'a, Vec<ResetValP>> {
    delimited("{", separated(1..,reset_val, ws(",")), "}")
        .context(StrContext::Label("reset array values"))
        .parse_next(input)
}

/// Parse possible field position format:
/// `msb:lsb` `lsb+:width` `5b` or `$width`
pub fn field_pos<'a>(input: &mut &'a str) -> Res<'a, FieldPos> {
    alt((
        separated_pair(ws(val_u8_or_param), ws(":"), val_u8_or_param).map(|v| FieldPos::MsbLsb((v.0, v.1))),
        separated_pair(ws(val_u8_or_param), ws("+:"), val_u8_or_param).map(|v| FieldPos::LsbSize((v.0, v.1))),
        delimited(multispace0, val_u8, "b").map( |v| FieldPos::Size(v.into())),
        ws(param).map(|v| FieldPos::Size(v.into())),
    ))
    .context(StrContext::Label("field position"))
    .parse_next(input)
}

// Field declaration format is the following
// - field_name[array_size] = rst_val pos&width attributes "field short description"
// where the (group_name) and description are optionnal
pub fn field_decl<'a>(input: &mut &'a str) -> Res<'a, Field> {
    let name = identifier(input)?;
    let array_size = opt(delimited("[", val_u8_or_param, "]")).parse_next(input)?;
    let reset_val = opt(preceded(
        ws("="),
        alt((
            reset_val.map(|v: ResetValP| vec![v]),
            reset_val_arr,
        )),
    ))
    .context(StrContext::Label("reset value"))
    .parse_next(input)?;
    let pos = field_pos(input)?;
    let kind = opt(field_sw_kind).parse_next(input)?;
    let desc = opt(ws(quoted_string)).parse_next(input)?;
    if !input.is_empty() {
        comment(input).map_err(|_| winnow::error::ErrMode::Incomplete(winnow::error::Needed::Unknown))?;
    }
    Ok(
        Field::new(
            name,
            reset_val.unwrap_or(vec![]),
            pos,
            kind,
            array_size,
            desc.unwrap_or(""),
        ),
    )
}

pub fn field_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
    terminated(
        alt((
            alt((
                alt((ws("description"), ws("desc"))).value(Context::Description),
                ws("enable.description").value(Context::DescIntrEnable),
                ws("mask.description").value(Context::DescIntrMask),
                ws("pending.description").value(Context::DescIntrPending),
                ws("swset").value(Context::SwSet),
                ws("pulse").value(Context::Pulse),
                ws("interrupt").value(Context::Interrupt),
                ws("hidden").value(Context::Hidden),
                alt((ws("disabled"), ws("disable"))).value(Context::Disabled),
                ws("reserved").value(Context::Reserved),
                ws("optional").value(Context::Optional),
                alt((ws("nbfrac"),ws("nb_frac"))).value(Context::FieldFrac),
            )),
            alt((
                ws("clock").value(Context::HwClock),
                ws(Caseless("clkEn")).value(Context::HwClkEn),
                ws("clear").value(Context::HwClear),
                ws("hwset").value(Context::HwSet),
                ws("hwclr").value(Context::HwClr),
                ws("hwtgl").value(Context::HwTgl),
                ws("hw").value(Context::HwAccess),
                ws("lock").value(Context::HwLock),
                ws("signed").value(Context::Signed),
                ws("toggle").value(Context::Toggle),
                ws("we").value(Context::HwWe),
                ws("wel").value(Context::HwWel),
                ws("counter").value(Context::Counter),
                ws("partial").value(Context::Partial),
                ws(Caseless("arrayPosIncr")).value(Context::ArrayPosIncr),
                ws(Caseless("arrayPartial")).value(Context::ArrayPartial),
                ws("enum").value(Context::Enum),
                ws("limit").value(Context::Limit),
                ws("password").value(Context::Password),
            )),
        )),
        opt(alt((ws(":"), space0))),
    )
    .context(StrContext::Label("field properties"))
    .parse_next(input)
}

pub fn field_acc<'a>(input: &mut &'a str) -> Res<'a, Access> {
    alt((
        ws(Caseless("na")).value(Access::NA),
        ws(Caseless("rw")).value(Access::RW),
        ws(Caseless("ro")).value(Access::RO),
        ws(Caseless("wo")).value(Access::WO),
        ws(Caseless("r")).value(Access::RO),
        ws(Caseless("w")).value(Access::WO),
    ))
    .context(StrContext::Label("field access"))
    .parse_next(input)
}

pub fn field_sw_kind<'a>(input: &mut &'a str) -> Res<'a, FieldSwKind> {
    alt((
        ws(Caseless("ro")).value(FieldSwKind::ReadOnly),
        ws(Caseless("rw")).value(FieldSwKind::ReadWrite),
        ws(Caseless("rclr")).value(FieldSwKind::ReadClr),
        ws(Caseless("wclr")).value(FieldSwKind::W1Clr),
        ws(Caseless("w1clr")).value(FieldSwKind::W1Clr),
        ws(Caseless("w0clr")).value(FieldSwKind::W0Clr),
        ws(Caseless("w1set")).value(FieldSwKind::W1Set),
        ws(Caseless("wo")).value(FieldSwKind::WriteOnly),
        ws(Caseless("pulse")).value(FieldSwKind::W1Pulse(false, false)),
        ws(Caseless("pulsereg")).value(FieldSwKind::W1Pulse(true, false)),
        ws(Caseless("toggle")).value(FieldSwKind::W1Tgl),
        ws(Caseless("r")).value(FieldSwKind::ReadOnly),
        ws(Caseless("w")).value(FieldSwKind::WriteOnly),
    ))
    .context(StrContext::Label("field kind"))
    .parse_next(input)
}

pub fn enum_kind<'a>(input: &mut &'a str) -> Res<'a, &'a str> {
    opt((ws(identifier),
        opt(preceded("::",identifier))
    )).take()
    .context(StrContext::Label("enum kind"))
    .parse_next(input)
}

pub fn clk_en(input: &str) -> ResF<'_, ClkEn> {
    let name = identifier.context(StrContext::Label("clock enable")).parse(input)?;
    if name.to_lowercase() == "false" {
        Ok(ClkEn::None)
    } else {
        Ok(ClkEn::Signal(name.to_owned()))
    }
}

// Format for an enum entry is :
// - name = value "description"
pub fn enum_entry(input: &str) -> ResF<'_, EnumEntry> {
    let info = (
        preceded(ws("-"), identifier),
        preceded(ws("="), val_u8),
        opt(delimited(ws("("),val_f64,ws(")"))),
        quoted_string,
    ).context(StrContext::Label("enum entry"))
    .parse(input)?;
    Ok(EnumEntry {
        name: info.0.to_owned(),
        value: info.1,
        repr: info.2,
        description: info.3.into(),
    })
}

pub fn field_interrupt<'a>(input: &mut &'a str) -> Res<'a, InterruptInfoField> {
    let mut info =
        permutation((
            opt(ws(reg_interrupt_trigger)),
            opt(ws(reg_interrupt_clr))
        )).context(StrContext::Label("interrupt info"))
        .parse_next(input)?;
    if info.1.is_some() && info.0.is_none() {
        let info_tmp = opt(ws(reg_interrupt_trigger)).context(StrContext::Label("interrupt trigger")).parse_next(input)?;
        if info_tmp.is_some() {
            info.0 = info_tmp;
        }
    }
    Ok(
        InterruptInfoField {
            trigger: info.0,
            clear: info.1,
        },
    )
}

/// Pulse kind can be 'reg' or 'comb'. Default to 'reg'.
pub fn pulse_kind(input: &str) -> ResF<'_, bool> {
    alt((
        ws("reg").value(true),
        ws("comb").value(false),
        space0.value(true),
    ))
    .context(StrContext::Label("pulse kind"))
    .parse(input)
}

// up|down|updown
pub fn counter_dir<'a>(input: &mut &'a str) -> Res<'a, CounterKind> {
    alt((
        ws("down").value(CounterKind::Down),
        ws("updown").value(CounterKind::UpDown),
        ws("up").value(CounterKind::Up),
    )).context(StrContext::Label("counter direction"))
    .parse_next(input)
}

// up|down|updown [incrVal[=width]] [decrVal[=width]] [sat] [event] [clr]
pub fn counter_def_<'a>(input: &mut &'a str) -> Res<'a, CounterInfo> {
    let kind = counter_dir.parse_next(input)?;
    // Extract incr/decr settings
    let mut val = permutation((
        opt(preceded(
            ws("incrVal"),
            opt(preceded(opt("="), val_u8)),
        )),
        opt(preceded(
            ws("decrVal"),
            opt(preceded(opt("="), val_u8)),
        )),
    )).context(StrContext::Label("counter incr/decr"))
    .parse_next(input)?;
    //
    if val.1.is_some() && val.0.is_none() {
        val.0 = opt(preceded(
            ws("incrVal"),
            opt(preceded("=", val_u8)),
        )).context(StrContext::Label("counter increment value"))
        .parse_next(input)?;
    }
    // Extract sat/event/clr (any order)
    let sig = if input.is_empty() {
        (vec![], "")
    } else {
        repeat_till(0..,
            opt(alt((
                ws("sat").value(0),
                ws("event").value(1),
                ws("clr").value(2),
            ))),
            winnow::combinator::eof,
        ).context(StrContext::Label("counter sat/event/clr"))
        .parse_next(input)?
    };
    let mut c = CounterInfo {
        kind,
        incr_val: val.0.unwrap_or(Some(0)).unwrap_or(1),
        decr_val: val.1.unwrap_or(Some(0)).unwrap_or(1),
        sat: false,
        event: false,
        clr: false,
    };
    sig.0.iter().for_each(|s| match s {
        Some(0) => c.sat = true,
        Some(1) => c.event = true,
        Some(2) => c.clr = true,
        _ => {}
    });
    Ok(c)
}

pub fn counter_def(input: &str) -> ResF<'_, CounterInfo> {
    counter_def_.parse(input)
}

// limit ([min:max]|{v0,v1,..}|enum) [bypass_signal]
pub fn limit_def(input: &str) -> ResF<'_, LimitP> {
    (
        alt((
            // Min/Max/MinMax
            delimited(
                ws("["),
                separated_pair(opt(reset_val), ws(":"), opt(reset_val)),
                ws("]"),
            ).map(|v| v.into()),
            // Arrays of values
            reset_val_arr.map(LimitValueP::List),
            // Enum
            ws("enum").value(LimitValueP::Enum),
        )),
        opt(ws(signal_name)),
    ).context(StrContext::Label("limit definition"))
        .parse(input)
        .map(|v| LimitP {
            value: v.0,
            bypass: v.1.unwrap_or_default().to_owned(),
        })
}

// password [once=<val>] [hold=<val>] [protect]
fn password_info_l<'a>(input: &mut &'a str) -> Res<'a,PasswordInfo> {
    let mut once = opt(preceded(ws("once="),reset_val))
        .context(StrContext::Label("once password"))
        .parse_next(input)?;
    let hold = opt(preceded(ws("hold="),reset_val))
        .context(StrContext::Label("hold password"))
        .parse_next(input)?;
    // Try again to parse "once" to handle permutation of both properties
    if once.is_none() {
        once = opt(preceded(ws("once="),reset_val))
            .context(StrContext::Label("once password"))
            .parse_next(input)?;
    }
    let protect = opt(ws("protect")).parse_next(input)?;
    let info = PasswordInfo { once, hold, protect: protect.is_some()};
    Ok(info)
}

pub fn password_info(input: &str) -> ResF<'_, PasswordInfo> {
    password_info_l.parse(input)
}

//------- TEST -------//

#[cfg(test)]
mod tests_parsing {

    use super::*;
    use crate::rifgen::{FieldSwKind, ResetValP, Width};

    #[test]
    fn test_reset_val() {
        assert_eq!(val_u8(&mut "34 "), Ok(34));
        assert_eq!(val_u8(&mut "0x34 "), Ok(0x34));
        assert_eq!(val_u8(&mut "2'b10 "), Ok(2));
        assert_eq!(val_u8(&mut "6'o10 "), Ok(8));
        assert_eq!(val_u8(&mut "8'd10 "), Ok(10));
        assert_eq!(val_u8(&mut "8'h1A "), Ok(26));
        assert_eq!(reset_val(&mut "34 "), Ok(ResetValP::Unsigned(34)));
        assert_eq!(reset_val(&mut "+34"), Ok(ResetValP::Signed(34)));
        assert_eq!(reset_val(&mut "34.0 "), Ok(ResetValP::FloatU(34.0)));
        assert_eq!(reset_val(&mut "+34.0"), Ok(ResetValP::FloatS(34.0)));
        assert_eq!(reset_val(&mut "10e3 "), Ok(ResetValP::FloatU(10e3)));
        assert_eq!(reset_val(&mut "+5E-4"), Ok(ResetValP::FloatS(5e-4)));
        assert_eq!(reset_val(&mut "-17 "), Ok(ResetValP::Signed(-17)));
        assert_eq!(reset_val(&mut "0x2A"), Ok(ResetValP::Unsigned(42)));
        assert_eq!(reset_val(&mut "32'ha9fe7032"), Ok(ResetValP::Unsigned(0xa9fe7032)));
        assert_eq!(reset_val(&mut "$param"), Ok(ResetValP::Param("param".to_owned())));
        assert_eq!(reset_val(&mut "enum_label"), Ok(ResetValP::Enum("enum_label".to_owned())));
        assert_eq!(
            reset_val_arr(&mut "{0, 1 , 0x2,0x3} rest of text"),
            Ok(vec![
                    ResetValP::Unsigned(0),
                    ResetValP::Unsigned(1),
                    ResetValP::Unsigned(2),
                    ResetValP::Unsigned(3)
                ]
            )
        );
        assert_eq!(
            reset_val_arr(&mut "{+0, -1}"),
            Ok(vec![ResetValP::Signed(0), ResetValP::Signed(-1)])
        );
        assert_eq!(
            reset_val_arr(&mut "{0, -1}"),
            Ok(vec![ResetValP::Unsigned(0), ResetValP::Signed(-1)])
        );
    }

    #[test]
    fn test_field_decl() {
        assert_eq!(
            field_decl(&mut "fieldname = 24  7: 0  \"Field description\""),
            Ok(
                Field {
                    name: "fieldname".to_owned(),
                    description: "Field description".into(),
                    pos: FieldPos::MsbLsb((Width::Value(7), Width::Value(0))),
                    reset: vec![ResetValP::Unsigned(24)],
                    array: Width::Value(0),
                    hw_acc: Access::RO,
                    ..Default::default()
                }
            )
        );
        assert_eq!(
            field_decl(&mut "field_ro  5b  \"Field Read-only\""),
            Ok(
                Field {
                    name: "field_ro".to_owned(),
                    description: "Field Read-only".into(),
                    pos: FieldPos::Size(Width::Value(5)),
                    reset: vec![ResetValP::Unsigned(0)],
                    array: Width::Value(0),
                    hw_acc: Access::WO,
                    sw_kind: FieldSwKind::ReadOnly,
                    ..Default::default()
                }
            )
        );
        assert_eq!(
            field_decl(&mut "field_constant = 0x3 10:5 ro  \"Field Constant\""),
            Ok(
                Field {
                    name: "field_constant".to_owned(),
                    description: "Field Constant".into(),
                    pos: FieldPos::MsbLsb((Width::Value(10), Width::Value(5))),
                    reset: vec![ResetValP::Unsigned(3)],
                    array: Width::Value(0),
                    hw_acc: Access::WO,
                    sw_kind: FieldSwKind::ReadOnly,
                    ..Default::default()
                }
            )
        );
        assert_eq!(
            field_decl(&mut "array[4] = {13,-37}  4+:8  \"Array 2 reset\""),
            Ok(
                Field {
                    name: "array".to_owned(),
                    description: "Array 2 reset".into(),
                    pos: FieldPos::LsbSize((Width::Value(4), Width::Value(8))),
                    reset: vec![ResetValP::Unsigned(13), ResetValP::Signed(-37)],
                    array: Width::Value(4),
                    hw_acc: Access::RO,
                    ..Default::default()
                }
            )
        );
    }

    #[test]
    fn test_enum_entry() {
        assert_eq!(
            enum_entry("- VAL0 = 5 \"F0 Value 0\""),
            Ok(EnumEntry {
                name: "VAL0".to_owned(),
                value: 5,
                repr: None,
                description: "F0 Value 0".into()
            })
        );
        assert_eq!(
            enum_entry("- VAL1 = 5 (5000) \"F1 Value 1\""),
            Ok(EnumEntry {
                name: "VAL1".to_owned(),
                value: 5,
                repr: Some(5000.0),
                description: "F1 Value 1".into()
            })
        );
        assert_eq!(
            enum_entry("- VAL2 = 1 (4e3) \"F2 Value 2\""),
            Ok(EnumEntry {
                name: "VAL2".to_owned(),
                value: 1,
                repr: Some(4e3),
                description: "F2 Value 2".into()
            })
        );
    }

    #[test]
    fn test_pulse_kind() {
        assert_eq!(pulse_kind("reg"), Ok(true));
        assert_eq!(pulse_kind("  "), Ok(true));
        assert_eq!(pulse_kind("comb"), Ok(false));
        assert!(pulse_kind("anything else").is_err());
    }

    // up|down|updown [incrVal[=width]] [decrVal[=width]] [sat] [event] [clr]
    #[test]
    fn test_counter_def() {
        assert_eq!(counter_dir(&mut "down"), Ok(CounterKind::Down));
        assert_eq!(counter_dir(&mut "updown"), Ok(CounterKind::UpDown));
        assert_eq!(counter_dir(&mut "up"), Ok(CounterKind::Up));
        assert_eq!(
            counter_def("up decrVal incrVal=3"),
            Ok(CounterInfo {
                kind: CounterKind::Up,
                incr_val: 3,
                decr_val: 1,
                sat: false,
                event: false,
                clr: false
            })
        );
        assert_eq!(
            counter_def("up decrVal 2 sat"),
            Ok(CounterInfo {
                kind: CounterKind::Up,
                incr_val: 0,
                decr_val: 2,
                sat: true,
                event: false,
                clr: false
            })
        );
        assert_eq!(
            counter_def("down clr event sat"),
            Ok(CounterInfo {
                kind: CounterKind::Down,
                incr_val: 0,
                decr_val: 0,
                sat: true,
                event: true,
                clr: true
            })
        );
        assert_eq!(
            counter_def("updown clr"),
            Ok(CounterInfo {
                kind: CounterKind::UpDown,
                incr_val: 0,
                decr_val: 0,
                sat: false,
                event: false,
                clr: true
            })
        );
    }

    // limit ([min:max]|{v0,v1,..}|enum) [bypass_signal]
    #[test]
    fn test_limit() {
        assert_eq!(
            limit_def("[0:7] bypass"),
            Ok(LimitP {
                value: LimitValueP::MinMax(ResetValP::Unsigned(0), ResetValP::Unsigned(7)),
                bypass: "bypass".to_string()
            })
        );
        assert_eq!(
            limit_def("[:5]"),
            Ok(LimitP {
                value: LimitValueP::Max(ResetValP::Unsigned(5)),
                bypass: "".to_string()
            })
        );
        assert_eq!(
            limit_def("[3:]"),
            Ok(LimitP {
                value: LimitValueP::Min(ResetValP::Unsigned(3)),
                bypass: "".to_string()
            })
        );
        assert_eq!(
            limit_def("{3,5,9} allow"),
            Ok(LimitP {
                value: LimitValueP::List(vec![
                    ResetValP::Unsigned(3),
                    ResetValP::Unsigned(5),
                    ResetValP::Unsigned(9)
                ]),
                bypass: "allow".to_string()
            })
        );
        assert_eq!(
            limit_def("enum"),
            Ok(LimitP {
                value: LimitValueP::Enum,
                bypass: "".to_string()
            })
        );
    }

    // password [once=<val>] [hold=<val>] [protect]
    #[test]
    fn test_password() {
        assert_eq!(
            password_info("hold=0x1234"),
            Ok(PasswordInfo {
                once: None,
                hold: Some(ResetValP::Unsigned(0x1234)),
                protect: false,
            })
        );
        assert_eq!(
            password_info("once=0x1337"),
            Ok(PasswordInfo {
                hold: None,
                once: Some(ResetValP::Unsigned(0x1337)),
                protect: false,
            })
        );
        assert_eq!(
            password_info("hold=0xDEAD once=0xC0FE protect"),
            Ok(PasswordInfo {
                hold: Some(ResetValP::Unsigned(0xDEAD)),
                once: Some(ResetValP::Unsigned(0xC0FE)),
                protect: true,
            })
        );
        assert_eq!(
            password_info("once=0xBAD hold=0xBED"),
            Ok(PasswordInfo {
                once: Some(ResetValP::Unsigned(0xBAD)),
                hold: Some(ResetValP::Unsigned(0xBED)),
                protect: false,
            })
        );
    }

}
