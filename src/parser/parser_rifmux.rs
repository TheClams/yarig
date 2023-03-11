use winnow::{
  combinator::{alt, delimited, opt, preceded, terminated}, error::ErrorKind, Parser
};

use crate::rifgen::{AddressKind, AddressOffset, Context, RifmuxItem, RifType, RifmuxGroup, SuffixInfo};

use super::{identifier, param, path_name, quoted_string, val_u64, val_u8, ws, Res, ResF};


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
      ws("map"        ).value(Context::RifmuxMap  ),
      ws("top"        ).value(Context::RifmuxTop  ),
    )),
    ws(":")
  ).parse_next(input)
}

pub fn rifmux_map<'a>(input: &mut &'a str) -> Res<'a, Context> {
  alt((
    (ws("group"),opt(":")).value(Context::RifmuxGroup),
    ws("-").value(Context::Item("".to_owned()))
  )).parse_next(input)
}

pub fn address_offset<'a>(input: &mut &'a str) -> Res<'a, AddressOffset> {
    alt((
      val_u64.try_map(|v| -> Result<AddressOffset, ErrorKind> {Ok( AddressOffset::Value(v))}),
      ws(param).try_map(|v| -> Result<AddressOffset, ErrorKind> {Ok(AddressOffset::Param(v.to_owned()))}),
    )).parse_next(input)
}

// - <rif_name> = <rif_type> @ <regAddr> "description"
// - <rif_name> external <addrWidth> @ <regAddr> "description"
pub fn rif_inst<'a>(input: &'a str, group: &'a str) -> ResF<'a, RifmuxItem> {
  (
    ws(identifier),
    alt((
      preceded(ws("="), ws(identifier)).try_map(|s| -> Result<RifType,ErrorKind> {Ok(RifType::Rif(s.to_owned()))}),
      preceded(ws("external"),val_u8).try_map(|s| -> Result<RifType,ErrorKind> {Ok(RifType::Ext(s))}),
    )),
    opt((
        alt((
          ws("@+=").value(AddressKind::RelativeSet),
          ws("@+").value(AddressKind::Relative),
          ws("@").value(AddressKind::Absolute)
        )),
        address_offset
    )),
    opt(quoted_string)
  ).parse(input).map(|v| RifmuxItem::new(v, group))
}

pub fn rif_inst_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
  terminated(
    alt((
      ws("description").value(Context::Description),
      ws("desc"       ).value(Context::Description),
      ws("parameters" ).value(Context::Parameters ),
      ws("suffix"     ).value(Context::Suffix     ),
    )),
    ws(":")
  ).parse_next(input)
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
  ).parse_next(input)
  .map(|v|
    SuffixInfo::new(
      v.0.to_owned(),
      matches!(v.1,Some((true,_))),
      matches!(v.1,Some((_,true))))
  )
}

pub fn suffix_info(input: &str) -> ResF<SuffixInfo> {
    suffix_info_l.parse(input)
}

pub fn rif_inst_suffix(input: &str) -> ResF<(Option<&str>, SuffixInfo)> {
  (
    opt(terminated(path_name,"=")),
    suffix_info_l
  ).parse(input)
}

pub fn rifmux_group(input: &str) -> ResF<RifmuxGroup> {
  (
    ws(identifier),
    alt((
      ws("@+=").value(AddressKind::RelativeSet),
      ws("@+").value(AddressKind::Relative),
      ws("@").value(AddressKind::Absolute)
    )),
    address_offset,
    opt(quoted_string)
  ).parse(input)
  .map(|v| v.into())
}


#[cfg(test)]
mod tests_parsing {

    use super::*;
    // use crate::rifgen::{EnumKind,FieldHwKind,FieldSwKind, ResetVal, Visibility};

    #[test]
    fn test_suffix_info() {
        assert_eq!(
          suffix_info(&mut "name_only"),
          Ok(SuffixInfo { name: "name_only".to_owned(), alt_pos: false, pkg: false })
        );
        assert_eq!(
          suffix_info(&mut "ctrl(pkg)"),
          Ok(SuffixInfo { name: "ctrl".to_owned(), alt_pos: false, pkg: true })
        );
        assert_eq!(
          suffix_info(&mut "name(alt)"),
          Ok(SuffixInfo { name: "name".to_owned(), alt_pos: true, pkg: false })
        );
        assert_eq!(
          suffix_info(&mut "n1(alt,pkg)"),
          Ok(SuffixInfo { name: "n1".to_owned(), alt_pos: true, pkg: true })
        );
        assert_eq!(
          suffix_info(&mut "n2(pkg,alt)"),
          Ok(SuffixInfo { name: "n2".to_owned(), alt_pos: true, pkg: true })
        );
    }
}