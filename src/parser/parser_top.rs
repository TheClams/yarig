use crate::rifgen::{Context, Interface, ResetDef, GenericRange};

use winnow::{
  ascii::Caseless, combinator::{alt, opt, preceded, separated_pair, terminated}, error::StrContext, Parser
};

use super::{identifier, item_cntxt, parse_binary, parse_range, quoted_string, ws, Res, ResF};

//--------------------------------
// Top Level
fn decl_rif<'a>(input: &mut &'a str) -> Res<'a, Context> {
  ("rif", ws(":")).value(Context::Rif).parse_next(input)
}

fn decl_rifmux<'a>(input: &mut &'a str) -> Res<'a, Context> {
  ("rifmux", ws(":")).value(Context::Rifmux).parse_next(input)
}

pub fn decl_top<'a>(input: &mut &'a str) -> Res<'a, (Context, &'a str)> {
  (
    alt((decl_rif,decl_rifmux)),
    identifier
  ).parse_next(input)
}

//--------------------------------
// RIF Properties

pub fn rif_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
  terminated(
  	alt((
    	ws("description").value(Context::Description),
    	ws("desc"       ).value(Context::Description),
    	ws("parameters" ).value(Context::Parameters ),
      ws("generics"   ).value(Context::Generics   ),
    	ws("info"       ).value(Context::Info       ),
    	ws("interface"  ).value(Context::Interface  ),
    	ws("addrWidth"  ).value(Context::AddrWidth  ),
    	ws("dataWidth"  ).value(Context::DataWidth  ),
    	ws("swClock"    ).value(Context::SwClock    ),
    	ws("hwClock"    ).value(Context::HwClock    ),
    	ws("swClkEn"    ).value(Context::SwClkEn    ),
    	ws("hwClkEn"    ).value(Context::HwClkEn    ),
    	ws("swReset"    ).value(Context::SwReset    ),
    	ws("hwReset"    ).value(Context::HwReset    ),
    	ws("hwClear"    ).value(Context::HwClear    ),
    	ws("swClear"    ).value(Context::SwClear    ),
      alt((
        ws("suffixPkg"),
        ws("suffix_pkg")
      )).value(Context::SuffixPkg),
    	item_cntxt,
  	)),
    ws(":")
  ).context(StrContext::Label("rif property"))
  .parse_next(input)
}

pub fn rif_properties_or_item<'a>(input: &mut &'a str) -> Res<'a, Context> {
  alt((rif_properties, item_cntxt)).parse_next(input)
}

pub fn val_intf<'a>(input: &mut &'a str) -> Res<'a, Interface> {
  identifier.map(Interface::from).parse_next(input)
}


// Format is name [[active]Low|High] [async|sync]
// Default is activeLow async
pub fn reset_def(input: &str) -> ResF<ResetDef> {
  (
    ws(identifier),
    opt(
    	preceded(
    		opt(ws("active")),
    		alt((ws(Caseless("Low")),ws(Caseless("High"))))
    	)),
    opt(alt((ws("async"),ws("sync")))),
  ).context(StrContext::Label("reset definition"))
  .parse(input)
  .map(|info| ResetDef{
      name: info.0.to_owned(),
      active_high: info.1 == Some("High") || info.1 == Some("high"),
      sync: info.2 == Some("sync"),
    })
}

pub fn generic_range<'a>(input: &mut &'a str) -> Res<'a, GenericRange> {
  let values = alt((
      parse_range,
      parse_binary.map(|v| vec![v]),
    )).context(StrContext::Label("generic range"))
    .parse_next(input)?;
  let desc = opt(quoted_string)
    .context(StrContext::Label("generic description"))
    .parse_next(input)?;
  Ok((values, desc).into())
}


pub fn generic_def(input: &str) -> ResF<(&str, GenericRange)> {
    preceded(
        "-",
        separated_pair(
            ws(identifier),
            opt(alt(("=", ":"))),
            generic_range,
        ),
    ).context(StrContext::Label("generic definition"))
    .parse(input)
}

//--------------------------------
// Tests

#[cfg(test)]
mod tests_parsing {

	use super::*;

  #[test]
  fn test_parse_top() {
    assert_eq!(decl_rif(&mut "rif :"), Ok(Context::Rif));
    assert_eq!(decl_rif(&mut "rif:"), Ok(Context::Rif));
    assert_eq!(decl_rifmux(&mut "rifmux:  "), Ok(Context::Rifmux));
    assert_eq!(decl_top(&mut "rifmux: rifMuxName"), Ok((Context::Rifmux, "rifMuxName")));
    assert_eq!(decl_top(&mut "rif : rifName"), Ok((Context::Rif, "rifName")));
  }

  #[test]
  fn test_rif_properties() {
    assert_eq!(rif_properties(&mut "addrWidth : 32"), Ok(Context::AddrWidth));
    assert_eq!(rif_properties(&mut "dataWidth: 9"  ), Ok(Context::DataWidth));
    assert_eq!(rif_properties(&mut "description: text with 9 and €"), Ok(Context::Description));
    assert_eq!(rif_properties(&mut "info: "), Ok(Context::Info));
  }


  #[test]
  fn test_val_intf() {
    assert_eq!(val_intf(&mut "Default "), Ok(Interface::Default) );
    assert_eq!(val_intf(&mut "apb"), Ok(Interface::Apb));
    assert_eq!(val_intf(&mut "Apb "), Ok(Interface::Apb));
    assert_eq!(val_intf(&mut "my_intf5"), Ok(Interface::Custom("my_intf5".to_owned())));
    assert!(val_intf(&mut "543 ").is_err());
    // assert_eq!(val_intf(&mut "543 "), Err(ErrMode::Backtrack(winnow::error::InputError{input:"543 ", kind:ErrorKind::Tag})) );
    assert!(val_intf(&mut "// bad ").is_err());
  }

  #[test]
  fn test_reset_def() {
    assert_eq!(reset_def("default"), Ok(ResetDef {name:"default".to_owned(),sync:false,active_high:false}) );
    assert_eq!(reset_def("low_async low async"), Ok(ResetDef {name:"low_async".to_owned(),sync:false,active_high:false}) );
    assert_eq!(reset_def("high_async high"), Ok(ResetDef {name:"high_async".to_owned(),sync:false,active_high:true}) );
    assert_eq!(reset_def("high_sync sync"), Ok(ResetDef {name:"high_sync".to_owned(),sync:true,active_high:false}) );
    assert_eq!(reset_def("activeH activeHigh"), Ok(ResetDef {name:"activeH".to_owned(),sync:false,active_high:true}) );
    assert_eq!(reset_def("activeL activeLow"), Ok(ResetDef {name:"activeL".to_owned(),sync:false,active_high:false}) );
    assert!(reset_def("error invalid option").is_err());
  }

  #[test]
  fn test_generic_range() {
    assert_eq!(generic_range(&mut "false"), Ok(GenericRange {min:0, max:1, default:0, desc: None}));
    assert_eq!(generic_range(&mut "true"), Ok(GenericRange {min:0, max:1, default:1, desc: None}));
    assert_eq!(generic_range(&mut "8"), Ok(GenericRange {min:1, max:8, default:8, desc: None}));
    assert_eq!(generic_range(&mut "4:5 \"Range 4 to 5\""), Ok(GenericRange {min:1, max:5, default:4, desc: Some("Range 4 to 5".to_owned())}));
    assert_eq!(generic_range(&mut "1:7:16"), Ok(GenericRange {min:1, max:16, default:7, desc: None}));
  }

}