use winnow::{
  combinator::{alt, delimited, opt, preceded, terminated},
  error::StrContext, Parser
};

use crate::rifgen::{Context, RifmuxItem, RifType, RifmuxGroup, SuffixInfo};

use super::{Res, ResF, address, identifier, path_name, quoted_string, val_u8, ws};


pub fn rifmux_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
  terminated(
    alt((
      ws("description").value(Context::Description),
      ws("desc"       ).value(Context::Description),
      ws("info"       ).value(Context::Info       ),
      ws("swClock"    ).value(Context::SwClock    ),
      ws("swClkEn"    ).value(Context::SwClkEn    ),
      ws("swReset"    ).value(Context::SwReset    ),
      ws("interface"  ).value(Context::Interface  ),
      ws("addrWidth"  ).value(Context::AddrWidth  ),
      ws("dataWidth"  ).value(Context::DataWidth  ),
      ws("parameters" ).value(Context::Parameters ),
      ws("generics"   ).value(Context::Generics   ),
      ws("map"        ).value(Context::RifmuxMap  ),
      ws("top"        ).value(Context::RifmuxTop  ),
    )),
    ws(":")
  ).context(StrContext::Label("rifmux property"))
  .parse_next(input)
}

pub fn rifmux_map<'a>(input: &mut &'a str) -> Res<'a, Context> {
  alt((
    (ws("group"),opt(":")).value(Context::RifmuxGroup),
    ws("-").value(Context::Item("".to_owned()))
  )).context(StrContext::Label("rifmux element"))
  .parse_next(input)
}

// - <rif_name> = <rif_type> @ <regAddr> "description"
// - <rif_name> external <addrWidth> @ <regAddr> "description"
pub fn rif_inst<'a>(input: &'a str, group: &'a str) -> ResF<'a, RifmuxItem> {
  (
    ws(identifier),
    alt((
      preceded(ws("="), ws(identifier)).map(|s| RifType::Rif(s.to_owned())),
      preceded(ws("external"),val_u8).map(RifType::Ext),
    )),
    opt(address),
    opt(quoted_string)
  ).context(StrContext::Label("rif instance"))
  .parse(input).map(|v| RifmuxItem::new(v, group))
}

pub fn rif_inst_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
  terminated(
    alt((
      ws("description").value(Context::Description),
      ws("desc"       ).value(Context::Description),
      ws("parameters" ).value(Context::Parameters ),
      ws("suffix"     ).value(Context::Suffix     ),
      ws("optional").value(Context::Optional),
    )),
    ws(":")
  ).context(StrContext::Label("rifinstance property"))
  .parse_next(input)
}

pub fn suffix_info_l<'a>(input: &mut &'a str) -> Res<'a, SuffixInfo> {
  (identifier,
    opt(
      delimited('(',
        alt((
          (ws("alt"),",",ws("pkg")).value((true,true)),
          (ws("pkg"),",",ws("alt")).value((true,true)),
          ws("alt").value((true,false)),
          ws("pkg").value((false,true)),
        )),
      ')')
    )
  ).context(StrContext::Label("suffix definition"))
  .parse_next(input)
  .map(|v|
    SuffixInfo::new(
      v.0.to_owned(),
      matches!(v.1,Some((true,_))),
      matches!(v.1,Some((_,true))))
  )
}

pub fn suffix_info(input: &str) -> ResF<'_, SuffixInfo> {
    suffix_info_l.parse(input)
}

pub fn rif_inst_suffix(input: &str) -> ResF<'_, (Option<&str>, SuffixInfo)> {
  (
    opt(terminated(path_name,"=")),
    suffix_info_l
  ).parse(input)
}

pub fn rifmux_group(input: &str) -> ResF<'_, RifmuxGroup> {
  (
    ws(identifier),
    address,
    opt(quoted_string)
  ).context(StrContext::Label("rifmux group definition"))
  .parse(input)
  .map(|v| v.into())
}


#[cfg(test)]
mod tests_parsing {

    use super::*;
    // use crate::rifgen::{EnumKind,FieldHwKind,FieldSwKind, ResetVal, Visibility};

    #[test]
    fn test_suffix_info() {
        assert_eq!(
          suffix_info("name_only"),
          Ok(SuffixInfo { name: "name_only".to_owned(), alt_pos: false, pkg: false })
        );
        assert_eq!(
          suffix_info("ctrl(pkg)"),
          Ok(SuffixInfo { name: "ctrl".to_owned(), alt_pos: false, pkg: true })
        );
        assert_eq!(
          suffix_info("name(alt)"),
          Ok(SuffixInfo { name: "name".to_owned(), alt_pos: true, pkg: false })
        );
        assert_eq!(
          suffix_info("n1(alt,pkg)"),
          Ok(SuffixInfo { name: "n1".to_owned(), alt_pos: true, pkg: true })
        );
        assert_eq!(
          suffix_info("n2(pkg,alt)"),
          Ok(SuffixInfo { name: "n2".to_owned(), alt_pos: true, pkg: true })
        );
    }
}