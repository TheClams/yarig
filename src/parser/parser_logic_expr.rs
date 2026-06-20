use winnow::{combinator::alt, Parser};

// use crate::{error::RifError, parser::ws};
use crate::{error::RifError, parser::ws, rifgen::{LogicExpr, SignalRange}};

use super::{identifier, val_isize, Res};


#[derive(Clone, Copy, PartialEq, Debug)]
enum OpKind {
    /// Logical Not !
    NotL,
    /// Binary Not ~
    NotB,
    /// Logical Or ||
    OrL,
    /// Binary Or |
    OrB,
    /// Logical And &&
    AndL,
    /// Binary And &
    AndB,
    /// Xor operator ^
    Xor,
    /// Equality test ==
    Equal,
    /// Not Equal test !=
    NotEqual,
    /// Greater than or equal >=
    Gte,
    /// Lesser Than or equal <=
    Lte,
    /// Lesser than <
    Lt,
    /// Greater than >
    Gt,
    /// Access path operator .
    Dot,
}

impl OpKind {
    fn precedence(&self) -> u8 {
        match self {
            OpKind::Dot => 1,
            // Single operand operator
            OpKind::NotL |
            OpKind::NotB => 2,
            // Boolean operator
            OpKind::OrL  |
            OpKind::OrB  |
            OpKind::AndL |
            OpKind::AndB |
            OpKind::Xor  => 3,
            // Comparison
            OpKind::Equal    |
            OpKind::NotEqual => 7,
            OpKind::Gte |
            OpKind::Lte |
            OpKind::Lt  |
            OpKind::Gt  => 6,
        }
    }

    fn priority(&self, op: &OpKind) -> bool {
        self.precedence() < op.precedence()
    }
}


#[derive(Clone, PartialEq, Debug)]
enum Token {
    Name(String),
    Operator(OpKind),
    ParenL, ParenR,
    BracketL, BracketR, Colon(u16),
    CurlyL, CurlyR, Comma(u16),
    Number(isize),
}


fn operator_two<'a>(input: &mut &'a str) -> Res<'a, Token> {
    use Token::Operator;
    use OpKind::*;
    alt((
        alt((
            ws("&&").value(Operator(AndL)),
            ws("&").value(Operator(AndB)),
            ws("||").value(Operator(OrL)),
            ws("|").value(Operator(OrB)),
            ws("==").value(Operator(Equal)),
            ws("!=").value(Operator(NotEqual)),
        )),
        alt((
            ws(">=").value(Operator(Gte)),
            ws("<=").value(Operator(Lte)),
            ws(">").value(Operator(Gt)),
            ws("<").value(Operator(Lt)),
            ws("!").value(Operator(NotL)),
            ws("~").value(Operator(NotB)),
            ws("^").value(Operator(Xor)),
        ))
    )).parse_next(input)
}

fn dot<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(".").value(Token::Operator(OpKind::Dot)).parse_next(input)
}

fn operator_one<'a>(input: &mut &'a str) -> Res<'a, Token> {
    use Token::Operator;
    use OpKind::*;
    alt((
        ws("!").value(Operator(NotL)),
        ws("~").value(Operator(NotB)),
    )).parse_next(input)
}

fn paren_l<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws("(").value(Token::ParenL).parse_next(input)
}

fn bracket_l<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws("[").value(Token::BracketL).parse_next(input)
}

fn curly_l<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws("{").value(Token::CurlyL).parse_next(input)
}

fn paren_r<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(")").value(Token::ParenR).parse_next(input)
}

fn bracket_r<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws("]").value(Token::BracketR).parse_next(input)
}

fn curly_r<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws("}").value(Token::CurlyR).parse_next(input)
}

fn colon<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(":").value(Token::Colon(0)).parse_next(input)
}

fn comma<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(",").value(Token::Comma(0)).parse_next(input)
}

fn number<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(val_isize).map(Token::Number).parse_next(input)
}

fn name<'a>(input: &mut &'a str) -> Res<'a, Token> {
    identifier.map(|n| Token::Name(n.to_owned())).parse_next(input)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ExprState {
    Operand,
    Operator
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ExprContext {
    Group,
    Range(u16),
    Concat(u16),
}


pub fn parse_logic_expr(input: &str) -> Result<LogicExpr,RifError> {
    let mut tokens : Vec<Token> = Vec::with_capacity(2);
    let mut op_stack : Vec<Token> = Vec::with_capacity(2);
    let mut cntxt : Vec<ExprContext> = Vec::with_capacity(1);
    let mut state = ExprState::Operand;
    let mut s = input;

    while !s.is_empty() {

        let token = match state {
            ExprState::Operand => alt((name, number, paren_l, curly_l, operator_one, dot)).parse_next(&mut s)?,
            ExprState::Operator => match cntxt.last() {
                None => alt((operator_two, dot, bracket_l)).parse_next(&mut s)?,
                Some(ExprContext::Group)  => alt((operator_two, dot, bracket_l, paren_r)).parse_next(&mut s)?,
                Some(ExprContext::Range(_))  => alt((colon, bracket_r)).parse_next(&mut s)?,
                Some(ExprContext::Concat(_)) => alt((operator_two, dot, bracket_l, comma, curly_r)).parse_next(&mut s)?,
            }
        };
        // println!("Context = {:?} | token = {:?}", cntxt, token);

        match token {
            Token::Number(_) |
            Token::Name(_) => {
                tokens.push(token);
                state = ExprState::Operator;
            }
            // Single operand operator: push on the stack
            Token::Operator(OpKind::NotL) |
            Token::Operator(OpKind::NotB) => op_stack.push(token),
            // Operator -> Move operator stack to output until higher precedence operator is found
            // and then push operator to the stack, and change state to operand
            Token::Operator(op_r) => {
                while let Some(t) = op_stack.last() {
                    match t {
                        Token::Operator(op_l) if !op_r.priority(op_l) => tokens.push(op_stack.pop().unwrap()),
                        _ => break,
                    }
                }
                op_stack.push(token);
                state = ExprState::Operand;
            }
            //-------
            // Group
            Token::ParenL => {
                cntxt.push(ExprContext::Group);
                op_stack.push(Token::ParenL);
            }
            // Closing parenthesis : Pop last context and pop operators stack
            Token::ParenR => {
                cntxt.pop();
                while let Some(op) = op_stack.pop() {
                    match op {
                        Token::ParenL => break,
                        _ => tokens.push(op),
                    }
                }
            }
            // Range
            Token::BracketL => {
                if let Some(Token::Operator(OpKind::Dot)) = op_stack.last() {
                    tokens.push(op_stack.pop().unwrap());
                }
                cntxt.push(ExprContext::Range(1));
                op_stack.push(Token::BracketL);
                state = ExprState::Operand;
            }
            Token::BracketR => {
                let Some(ExprContext::Range(n)) = cntxt.pop() else {
                    return Err(RifError::generic("Malformed expression: unexpected ']'"));
                };
                tokens.push(Token::Colon(n));
                while let Some(op) = op_stack.pop() {
                    match op {
                        Token::BracketL => break,
                        _ => tokens.push(op),
                    }
                }
            }
            // Colon is used in range separator: maybe add a change in context to indicate this is the second part ?
            Token::Colon(_) => {
                state = ExprState::Operand;
                if let Some(ExprContext::Range(n)) = cntxt.last_mut() {
                    *n += 1;
                } else {
                    return Err(RifError::generic("Malformed expression: unexpected ':'"));
                }
            },
            // Concatenation
            Token::CurlyL => {
                cntxt.push(ExprContext::Concat(0));
                op_stack.push(Token::CurlyL);
                state = ExprState::Operand;
            }
            Token::CurlyR => {
                cntxt.pop();
                if let Some(ExprContext::Concat(n)) = cntxt.last_mut() {
                    tokens.push(Token::Comma(*n));
                } else {
                    return Err(RifError::generic("Malformed expression: unexpected '}'"));
                }

                while let Some(op) = op_stack.pop() {
                    match op {
                        Token::CurlyL => break,
                        _ => tokens.push(op),
                    }
                }
            }
            Token::Comma(_) => {
                state = ExprState::Operand;
                if let Some(ExprContext::Concat(n)) = cntxt.last_mut() {
                    *n += 1;
                }
            }
        }
    }
    // Empty the operator stack once all tokens have been parsed
    while let Some(op) = op_stack.pop() {
        tokens.push(op);
    }
    // println!("{tokens:?}");
    // Ok(tokens)
    let mut expr : Vec<LogicExpr> = Vec::with_capacity(tokens.len());
    for token in tokens {
        // println!("expr={expr:?} token = {token:?}");
        match token {
            Token::Name(n) => expr.push(LogicExpr::Id(n.into())),
            Token::Number(n) => expr.push(LogicExpr::ValueI(n as i128,32)),
            Token::Operator(OpKind::NotL) => {
                let e = expr.pop().ok_or(RifError::generic("Malformed expression: unexpected '!'"))?;
                expr.push(LogicExpr::Not(e.into()));
            }
            Token::Operator(OpKind::NotB) => {
                let e = expr.pop().ok_or(RifError::generic("Malformed expression: unexpected '~'"))?;
                expr.push(LogicExpr::NotB(e.into()));
            }
            Token::Operator(OpKind::Dot) => {
                let Some(LogicExpr::Id(mut id_r)) = expr.pop() else {return Err(RifError::generic("Malformed path !"));};
                match expr.pop() {
                    Some(LogicExpr::Id(mut id_l)) => {
                        if id_l.field.is_some() || id_r.field.is_some() {return Err(RifError::generic("Malformed path !"));}
                        id_l.field = Some(id_r.name);
                        expr.push(LogicExpr::Id(id_l));
                    }
                    // Path in the form .name => input path
                    _ => {
                        id_r.field = Some(id_r.name.clone());
                        id_r.name = "".to_owned();
                        expr.push(LogicExpr::Id(id_r));
                    }
                }
            }
            Token::Operator(op_kind) => {
                let err = RifError::generic(&format!("Malformed expression for operator {op_kind:?}"));
                let rhs = expr.pop().ok_or(err.clone())?;
                let mut lhs = expr.pop().ok_or(err.clone())?;
                match op_kind {
                    OpKind::OrL => {
                        if lhs.is_or() {
                            lhs.push(rhs);
                            expr.push(lhs);
                        } else {
                            expr.push(LogicExpr::Or(vec![lhs, rhs]));
                        }
                    }
                    OpKind::AndL     => {
                        if lhs.is_and() {
                            lhs.push(rhs);
                            expr.push(lhs);
                        } else {
                            expr.push(LogicExpr::And(vec![lhs, rhs]));
                        }
                    }
                    OpKind::OrB      => expr.push(LogicExpr::OrB(lhs.into(), rhs.into())),
                    OpKind::AndB     => expr.push(LogicExpr::AndB(lhs.into(), rhs.into())),
                    OpKind::Xor      => expr.push(LogicExpr::Xor(lhs.into(), rhs.into())),
                    OpKind::Equal    => expr.push(LogicExpr::Eq(lhs.into(), rhs.into())),
                    OpKind::NotEqual => expr.push(LogicExpr::Neq(lhs.into(), rhs.into())),
                    OpKind::Gte      => expr.push(LogicExpr::Gte(lhs.into(), rhs.into())),
                    OpKind::Lte      => expr.push(LogicExpr::Lte(lhs.into(), rhs.into())),
                    OpKind::Lt       => expr.push(LogicExpr::Lt(lhs.into(), rhs.into())),
                    OpKind::Gt       => expr.push(LogicExpr::Gt(lhs.into(), rhs.into())),
                    OpKind::Dot      => {
                        let (LogicExpr::Id(mut id_l), LogicExpr::Id(id_r)) = (lhs, rhs) else {return Err(err);};
                        if id_l.field.is_some() || id_r.field.is_some() {return Err(err);}
                        id_l.field = Some(id_r.name);
                        expr.push(LogicExpr::Id(id_l));
                    }
                    op => unreachable!("Operator {op:?} should already be handled")
                }
            },
            Token::Colon(n) => {
                let err = RifError::generic("Malformed signal range");
                if expr.len() < n as usize + 1 {
                    return Err(err);
                }
                let r = match n {
                    1 => {
                        let Some(LogicExpr::ValueI(idx,32)) = expr.pop() else {return Err(err);};
                        SignalRange::new_bit(idx as u8)
                    }
                    2 => {
                        let Some(LogicExpr::ValueI(lsb,32)) = expr.pop() else {return Err(err);};
                        let Some(LogicExpr::ValueI(msb,32)) = expr.pop() else {return Err(err);};
                        SignalRange::new(lsb as u8, msb as u8)
                    }
                    _ => {return Err(RifError::generic("Malformed signal range"));}
                };
                let Some(LogicExpr::Id(mut id)) = expr.pop() else {
                    return Err(RifError::generic("Malformed expression: unexpected '~'"));
                };
                if id.field.is_none() {
                    id.idx = Some(r.lsb() as u16)
                } else {
                    id.range = Some(r)
                }
                expr.push(LogicExpr::Id(id));
            }
            Token::Comma(n) => {
                if expr.len() < n as usize + 1 {
                    return Err(RifError::generic("Malformed concatenation"));
                }
                let c = expr.split_off(expr.len() - n as usize);
                expr.push(LogicExpr::Concat(c));
            }
            t => unreachable!("Unsupported token {t:?} in final vector")
        }
    }
    expr.pop().ok_or("Invalid logic expression".to_owned().into())
}


#[cfg(test)]
mod tests_parsing {
    use crate::rifgen::{ExprId, SignalRange};

    use super::*;

    #[test]
    fn test_parse_logic_expr() {
        assert_eq!(
            parse_logic_expr("256"),
            Ok(LogicExpr::ValueI(256, 32))
        );
        assert_eq!(
            parse_logic_expr("if_rif.address"),
            Ok(LogicExpr::Id(("if_rif","address").into()))
        );
        assert_eq!(
            parse_logic_expr(".ext_port"),
            Ok(LogicExpr::Id(("","ext_port").into()))
        );
        assert_eq!(
            parse_logic_expr("if_rif.address[2:0]==5"),
            Ok(LogicExpr::eq(
                ExprId::new_field_range(
                    "if_rif".to_owned(),
                    None,
                    "address".to_owned(),
                    Some(SignalRange::new(0, 2))
                ).into(),
                LogicExpr::ValueI(5, 32)))
        );
    }

}