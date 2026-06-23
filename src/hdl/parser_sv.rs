use winnow::{Parser, ascii::multispace1, combinator::{alt, delimited, opt, preceded, separated_pair, terminated}, error::{ContextError, ErrMode, StrContext}, token::take_until};

use crate::{hdl::{ParamKind, ParamType}, parser::parser_common::*};
use super::hw_info::{PortDir, PortInfo, ParamInfo, SignalDim, SignalKind, ParamDecl};

#[derive(Clone, Debug, PartialEq)]
pub enum VectorDim {
    /// Pair (msb,lsb)
    Range((String,String)),
    /// LSB + width
    LsbWidth((String,String)),
    /// MSB - Width
    MsbWidth((String,String)),
}

impl VectorDim {
    pub fn to_msb(self) -> Option<u16> {
        match self {
            VectorDim::Range((mut msb,_)) => {
                msb.retain(|c| !c.is_whitespace());
                if msb.ends_with("-1") {msb.parse::<u16>().ok()}
                else {msb.parse::<u16>().map(|x| x+1).ok()}
            },
            VectorDim::LsbWidth((_,w)) |
            VectorDim::MsbWidth((_,w)) => w.parse::<u16>().ok(),
        }
    }
}

/// Skip empty lines and comment block/lines
pub fn sv_skip_empty<'a>(input: &mut &'a str) -> Res<'a, () >{
    let mut p = Some("");
    while p.is_some() {
        p = opt(alt((sv_comment, multispace1))).parse_next(input)?;
    }
    Ok(())
}

/// Parse SystemVerilog module declaration
pub fn sv_module_decl<'a>(input: &mut &'a str)  -> Res<'a, (&'a str, Vec::<ParamInfo>, Vec::<PortInfo>) > {
    // Parse comments empty line before module declaration
    // TODO: might need to handle macro lines (`ifdef, `endif, ...) and maybe imports ?
    sv_skip_empty(input)?;
    let module_name = preceded( ws("module"), identifier).parse_next(input)?;
    let params = opt(sv_module_params).parse_next(input)?;
    let ports = sv_module_ports.parse_next(input)?;
    Ok((module_name, params.unwrap_or_default(), ports))
}

/// Parse SystemVerilog parameter declaration
pub fn sv_module_params<'a>(input: &mut &'a str)  -> Res<'a, Vec<ParamInfo>> {
    ws("#(").parse_next(input)?;
    let mut last = false;
    let mut params : Vec<ParamInfo> = Vec::new();
    let mut param = ParamInfo::default();
    while !last {
        sv_skip_empty(input)?;
        let decl: ParamDecl;
        (decl, last) = sv_param_decl.parse_next(input)?;
        param.set(decl);
        // println!("Parsed Param : {param:?} (last={last})");
        params.push(param.clone());
    }
    ws(")").context(StrContext::Label("End of params declaration")).parse_next(input)?;
    Ok(params)
}

/// Parse SystemVerilog port declaration
pub fn sv_module_ports <'a>(input: &mut &'a str)  -> Res<'a, Vec<PortInfo>> {
    // println!("Module ports on {:?}", input.lines().next());
    ws("(").parse_next(input)?;
    let mut last = false;
    let mut ports : Vec<PortInfo> = Vec::new();
    while !last {
        // println!("Parsing ports : checking for comment empty on {:?}", input.lines().next());
        sv_skip_empty(input)?;
        let p: PortInfo;
        // println!("Parsing ports : expecting port on {:?}", input.lines().next());
        (p, last) = alt((sv_port_logic, sv_port_intf)).parse_next(input)?;
        // println!("Parsed Port : {p:?} (last={last})");
        ports.push(p);
    }
    ws(")").context(StrContext::Label("End of ports declaration")).parse_next(input)?;
    ws(";").context(StrContext::Label("End of module declaration")).parse_next(input)?;
    Ok(ports)
}

pub fn sv_array_dim<'a>(input: &mut &'a str)  -> Res<'a, SignalDim> {
    delimited(
        ws("["),
        alt((
            val_u16.map(SignalDim::Fixed),
            identifier.map(|id| SignalDim::Generic(id.to_owned(),1)),
        )),
        ws("]")
    )
    .context(StrContext::Label("Array dimension"))
    .parse_next(input)
}

/// Parse a vectore dimension in the form [msb:lsb], [lsb+:width] or [msb-:width]
pub fn sv_vector_dim<'a>(input: &mut &'a str)  -> Res<'a, VectorDim> {
    delimited(
        ws("["),
        alt((sv_vector_dim_range,sv_vector_dim_idx_lsb,sv_vector_dim_idx_msb)),
        ws("]")
    )
    .context(StrContext::Label("Vector Dimension"))
    .parse_next(input)
}

/// Parse two expressions separated by :
fn sv_vector_dim_range<'a>(input: &mut &'a str)  -> Res<'a, VectorDim> {
    separated_pair(take_until(1..,":"), ":", take_until(1..,"]"))
    .map(|r:(&'a str, &'a str)| VectorDim::Range((r.0.to_owned(), r.1.to_owned())))
    .parse_next(input)
}

/// Parse two expressions separated by +:
fn sv_vector_dim_idx_lsb<'a>(input: &mut &'a str)  -> Res<'a, VectorDim> {
    separated_pair(take_until(1..,"+:"), "+:", take_until(1..,"]"))
    .map(|r:(&'a str, &'a str)| VectorDim::LsbWidth((r.0.to_owned(), r.1.to_owned())))
    .parse_next(input)
}

/// Parse two expressions separated by -:
fn sv_vector_dim_idx_msb<'a>(input: &mut &'a str)  -> Res<'a, VectorDim> {
    separated_pair(take_until(1..,"-:"), "-:", take_until(1..,"]"))
    .map(|r:(&str, &str)| VectorDim::MsbWidth((r.0.to_owned(), r.1.to_owned())))
    .parse_next(input)
}

/// Parse a custom type in the form of pkg::type or simply an identifier
pub fn sv_custom_type<'a>(input: &mut &'a str)  -> Res<'a, (Option<&'a str>,&'a str,&'a str)> {
    (opt(terminated(identifier,"::")),identifier,ws(identifier))
    .context(StrContext::Label("Custom type"))
    .parse_next(input)
}

/// Parse a input/output logic port
pub fn sv_port_logic<'a>(input: &mut &'a str)  -> Res<'a, (PortInfo, bool)> {
    let dir = alt((
        ws("input").value(PortDir::In),
        ws("output").value(PortDir::Out),
    )).context(StrContext::Label("Port direction"))
    .parse_next(input)?;
    let _net_type = opt(alt((ws("var"), ws("wire")))).parse_next(input)?;
    let is_logic = opt(alt((ws("bit"), ws("logic"), ws("reg")))).map(|x| x.is_some()).parse_next(input)?;
    let custom_type = if !is_logic {opt(sv_custom_type).parse_next(input)?} else {None};
    let is_signed = if is_logic {opt(alt((ws("signed"), ws("unsigned")))).map(|x: Option<&str>| x.unwrap_or_default().len()==6).parse_next(input)?} else {false};
    let vec_dim = opt(sv_vector_dim).context(StrContext::Label("Port dimension")).parse_next(input)?;
    // println!(" -> Direction = {dir:?}, net_type={_net_type:?}, is_logic={is_logic}, custom_type={custom_type:?}, dim={vec_dim:?}");
    let name = if let Some((_,_,n)) = &custom_type {n} else {ws(identifier).context(StrContext::Label("Port name")).parse_next(input)?};
    // println!(" -> name={name:?}");
    let arr_dim = opt(sv_array_dim).parse_next(input)?;
    // println!(" -> arr_dim={arr_dim:?}");
    let is_last = opt(ws(",")).map(|x| x.is_none()).parse_next(input)?;
    // println!(" -> is_last={is_last:?}");
    let desc = opt(sv_comment).parse_next(input)?.unwrap_or_default().trim().to_owned();
    // println!(" -> desc={desc:?}");
    //
    let kind = if let Some(t) = custom_type {
        SignalKind::Custom((t.0.map(|pkg| pkg.to_owned()), t.1.to_owned()))
    } else if ["data", "din", "dout"].iter().any(|&p| name.contains(p)) {SignalKind::Data}
    else if name.contains("addr") {SignalKind::Address}
    else {
        let w = vec_dim.map_or(Some(1), |d| d.to_msb()).ok_or(ErrMode::Cut(ContextError::new()))?;
        if is_signed {SignalKind::Signed(w)}
        else {SignalKind::Unsigned(w)}
    };
    // println!(" => kind={kind:?}");
    let port = PortInfo::new(name.to_owned(), kind, dir, arr_dim.unwrap_or_default(), desc);
    Ok((port, is_last))
}

/// Parse an interface port (format if_type.modport name)
pub fn sv_port_intf<'a>(input: &mut &'a str)  -> Res<'a, (PortInfo, bool)> {
    let intf_decl = (identifier,".",identifier, ws(identifier)).parse_next(input)?;
    let is_last = opt(ws(",")).map(|x| x.is_none()).parse_next(input)?;
    let desc = opt(sv_comment).parse_next(input)?.unwrap_or_default().trim().to_owned();
    let port = PortInfo::new_intf(intf_decl.3.to_owned(), intf_decl.0.to_owned(), intf_decl.2.to_owned(), desc);
    Ok((port, is_last))
}

/// Parse a parameter declaration
pub fn sv_param_decl<'a>(input: &mut &'a str)  -> Res<'a, (ParamDecl<'a>, bool)> {
    // Optional parameter/localparam
    let pkind = opt(alt((ws("parameter"), ws("localparam")))).parse_next(input)?;
    let kind = pkind.map(|s| if s.len()==10 {ParamKind::Local} else {ParamKind::Global});
    // Optional type
    let t = opt(alt((ws("int"), ws("logic"), ws("bit")))).parse_next(input)?;
    let ptype = match t {
        None => None,
        Some("int") => Some(ParamType::Int),
        Some(t) => {
            let vec_dim = opt(sv_vector_dim).context(StrContext::Label("Param vector dimension")).parse_next(input)?;
            let w = vec_dim.map_or(Some(1), |d| d.to_msb()).ok_or(ErrMode::Cut(ContextError::new()))?;
            if t.len() == 3 {Some(ParamType::Bit(w))} else {Some(ParamType::Logic(w))}
        }
    };
    // Parameter name
    let name = ws(identifier).parse_next(input)?;
    let val = opt(preceded(ws("="), val_isize)).parse_next(input)?;
    let is_last = opt(ws(",")).map(|x| x.is_none()).parse_next(input)?;
    let desc = opt(sv_comment).parse_next(input)?.map(|s| s.trim());
    Ok(((kind, ptype, name, val, desc), is_last))
}

/// Parse a comment in the form: /* ... */ or // ...
pub fn sv_comment<'a>(input: &mut &'a str)  -> Res<'a, &'a str> {
    alt((sv_comment_block, sv_comment_line)).parse_next(input)
}

/// Block comment in the form: /* ... */
pub fn sv_comment_block<'a>(input: &mut &'a str)  -> Res<'a, &'a str> {
    delimited(
        ws("/*"),
        take_until(0..,"*/"),
        "*/"
    ).parse_next(input)
}

/// Line comment in the form: // ...
pub fn sv_comment_line<'a>(input: &mut &'a str)  -> Res<'a, &'a str> {
    preceded(
        ws("//"),
        take_until(0..,"\n")
    ).parse_next(input)
}


//------- TEST -------//

#[cfg(test)]
mod tests_sv_parsing {

    use std::{path::PathBuf};

    use crate::{hdl::ModuleInfo, rifgen::Interface};

    fn check_module_info(info_ref: &ModuleInfo, parsed_ref: &ModuleInfo) {
        assert!(info_ref.name==parsed_ref.name, "Name mismatch");
        assert!(info_ref.params.len()==parsed_ref.params.len(), "Parameter number mismatch");
        assert!(info_ref.ports.len()==parsed_ref.ports.len(), "Port number mismatch");
        for param in parsed_ref.params.iter() {
            assert!(info_ref.params.contains(param), "Parameter mismatch");
        }
        // println!("Searching for {:#?}", info_ref.ports);
        for port in parsed_ref.ports.iter() {
            // println!("Searching for {port:?}");
            assert!(info_ref.ports.contains(port), "Port mismatch");
        }
    }

    #[test]
    fn test_sv_apbbridge_decl() {
        let mut path_file : PathBuf = env!("CARGO_MANIFEST_DIR").into();
        path_file.extend(["assets", "bridge_apb_rif.sv"]);
        let info = ModuleInfo::from_file(path_file).expect("Parsing bridge_apb_rif.sv should suceed");
        let bridge = ModuleInfo::new_bridge(&Interface::Apb, false);
        check_module_info(&bridge, &info);
    }

    #[test]
    fn test_sv_uauxbridge_decl() {
        let mut path_file : PathBuf = env!("CARGO_MANIFEST_DIR").into();
        path_file.extend(["assets", "bridge_uaux_rif.sv"]);
        let info = ModuleInfo::from_file(path_file).expect("Parsing bridge_uaux_rif.sv should suceed");
        let bridge = ModuleInfo::new_bridge(&Interface::Uaux, false);
        check_module_info(&bridge, &info);
    }

}
