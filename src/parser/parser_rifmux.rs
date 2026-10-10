use winnow::{
  combinator::{alt, delimited, opt, preceded, separated, terminated},
  error::StrContext, Parser
};

use crate::rifgen::{Context, ItemOptional, RifType, RifmuxGroup, RifmuxItem, SuffixInfo};

use super::{Res, ResF, address, identifier, path_name, quoted_string, val_u8, ws};


pub fn rifmux_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
  terminated(
    alt((
      alt((
        ws("description").value(Context::Description),
        ws("desc"       ).value(Context::Description),
        ws("info"       ).value(Context::Info       ),
        ws("swClock"    ).value(Context::SwClock    ),
        ws("swClkEn"    ).value(Context::SwClkEn    ),
        ws("swReset"    ).value(Context::SwReset    ),
        ws("interface"  ).value(Context::Interface  ),
      )),
      alt((
        ws("addrWidth"  ).value(Context::AddrWidth  ),
        ws("dataWidth"  ).value(Context::DataWidth  ),
        ws("parameters" ).value(Context::Parameters ),
        ws("generics"   ).value(Context::Generics   ),
        ws("map"        ).value(Context::RifmuxMap  ),
        ws("top"        ).value(Context::RifmuxTop  ),
        ws("enable"     ).value(Context::Enable     ),
      )),
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

/// Properties of a group (indented under the group declaration, next to its instances)
pub fn rifmux_group_properties<'a>(input: &mut &'a str) -> Res<'a, Context> {
  terminated(
    ws("enable").value(Context::Enable),
    ws(":")
  ).context(StrContext::Label("rifmux group property"))
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
      ws("enable"  ).value(Context::Enable  ),
    )),
    ws(":")
  ).context(StrContext::Label("rifinstance property"))
  .parse_next(input)
}

/// Flag following a suffix name
#[derive(Clone, Copy, Debug, PartialEq)]
enum SuffixFlag {Alt, Pkg, Doc, DocType}

fn suffix_flag<'a>(input: &mut &'a str) -> Res<'a, SuffixFlag> {
  alt((
    ws("alt").value(SuffixFlag::Alt),
    ws("pkg").value(SuffixFlag::Pkg),
    // doc_type must be tested before doc
    ws("doc_type").value(SuffixFlag::DocType),
    ws("doc").value(SuffixFlag::Doc),
  )).context(StrContext::Label("suffix flag (alt, pkg, doc or doc_type)"))
  .parse_next(input)
}

/// Parse a suffix definition: `name` or `name(flag0,flag1,...)` where each flag is one of alt, pkg, doc, doc_type
pub fn suffix_info_l<'a>(input: &mut &'a str) -> Res<'a, SuffixInfo> {
  (identifier,
    opt(delimited('(', separated(1.., suffix_flag, ","), ')'))
  ).context(StrContext::Label("suffix definition"))
  .parse_next(input)
  .map(|(name, flags): (&str, Option<Vec<SuffixFlag>>)| {
    let flags = flags.unwrap_or_default();
    SuffixInfo::new(
      name.to_owned(),
      flags.contains(&SuffixFlag::Alt),
      flags.contains(&SuffixFlag::Pkg),
    ).with_doc(
      flags.contains(&SuffixFlag::Doc),
      flags.contains(&SuffixFlag::DocType),
    )
  })
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

pub fn rif_inst_optional_en(input: &str) -> ResF<'_, ItemOptional> {
    ws("enable").value(ItemOptional::Enable).parse(input)
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
    fn test_enable_properties() {
        let mut l = "enable : false";
        assert_eq!(rifmux_properties(&mut l), Ok(Context::Enable));
        assert_eq!(l, "false");
        let mut l = "enable:";
        assert_eq!(rif_inst_properties(&mut l), Ok(Context::Enable));
        let mut l = "enable:";
        assert_eq!(rifmux_group_properties(&mut l), Ok(Context::Enable));
        let mut l = "- inst = rif_type @ 0x0";
        assert!(rifmux_group_properties(&mut l).is_err());
    }

    #[test]
    fn test_suffix_info() {
        assert_eq!(
          suffix_info("name_only"),
          Ok(SuffixInfo::new("name_only".to_owned(), false, false))
        );
        assert_eq!(
          suffix_info("ctrl(pkg)"),
          Ok(SuffixInfo::new("ctrl".to_owned(), false, true))
        );
        assert_eq!(
          suffix_info("name(alt)"),
          Ok(SuffixInfo::new("name".to_owned(), true, false))
        );
        assert_eq!(
          suffix_info("n1(alt,pkg)"),
          Ok(SuffixInfo::new("n1".to_owned(), true, true))
        );
        assert_eq!(
          suffix_info("n2(pkg,alt)"),
          Ok(SuffixInfo::new("n2".to_owned(), true, true))
        );
        assert_eq!(
          suffix_info("n3(doc)"),
          Ok(SuffixInfo::new("n3".to_owned(), false, false).with_doc(true, false))
        );
        assert_eq!(
          suffix_info("n4(doc_type)"),
          Ok(SuffixInfo::new("n4".to_owned(), false, false).with_doc(false, true))
        );
        assert_eq!(
          suffix_info("n5(alt, doc_type, pkg)"),
          Ok(SuffixInfo::new("n5".to_owned(), true, true).with_doc(false, true))
        );
        assert!(suffix_info("n6(unknown)").is_err());
        assert!(suffix_info("n7()").is_err());
    }

    #[test]
    fn test_suffix_apply() {
        use crate::rifgen::{SuffixTarget, SuffixTargets, SuffixTargetSel};
        let s = suffix_info("lite").unwrap();
        // Default: module and file names only
        assert_eq!(s.apply("uart_rif", SuffixTarget::RtlType), "uart_rif_lite");
        assert_eq!(s.apply("uart_rif", SuffixTarget::RtlPkg),  "uart_rif");
        assert_eq!(s.apply("uart_rif", SuffixTarget::DocFile), "uart_rif_lite");
        assert_eq!(s.apply("uart_rif", SuffixTarget::DocType), "uart_rif");
        // Flags enable extra targets, alt position places the suffix before _rif
        let s = suffix_info("lite(alt,pkg,doc_type)").unwrap();
        assert_eq!(s.apply("uart_rif", SuffixTarget::RtlPkg),  "uart_lite_rif");
        assert_eq!(s.apply("uart_rif", SuffixTarget::DocType), "uart_lite_rif");
        assert_eq!(s.apply("uart_rif", SuffixTarget::Sw),      "uart_lite_rif");
        // Global selection replaces the default targets
        let t = SuffixTargets::from_sel(&[SuffixTargetSel::Rtl, SuffixTargetSel::DocType]);
        let s = suffix_info("lite").unwrap().with_targets(t);
        assert_eq!(s.apply("uart", SuffixTarget::RtlType), "uart_lite");
        assert_eq!(s.apply("uart", SuffixTarget::RtlPkg),  "uart_lite");
        assert_eq!(s.apply("uart", SuffixTarget::DocType), "uart_lite");
        assert_eq!(s.apply("uart", SuffixTarget::DocFile), "uart");
        assert_eq!(s.apply("uart", SuffixTarget::Sw),      "uart");
        // Legacy option: only the RTL module
        let s = suffix_info("lite").unwrap().with_targets(SuffixTargets::rtl_only());
        assert_eq!(s.apply("uart", SuffixTarget::RtlType), "uart_lite");
        assert_eq!(s.apply("uart", SuffixTarget::DocFile), "uart");
        assert_eq!(s.apply("uart", SuffixTarget::Sim),     "uart");
    }
}