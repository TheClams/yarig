use std::{fmt::Display, ops::{Deref, DerefMut}};

use winnow::{ascii::{space0, Caseless}, combinator::{alt, delimited}, Parser};

use crate::{error::RifError, rifgen::order_dict::OrderDict};
use super::{identifier, val_f64, val_isize, ws, Res};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OpKind {
    /// Addition opearator +
    Plus,
    /// Subtraction opeartor
    Minus,
    /// Multiplication operator
    Mult,
    /// Division operator /
    Div,
    /// Remainder operator %
    Rem,
    /// Power operator: ^
    Pow,
    /// Not operator: 'not x' , '!x', '~x'
    Not,
    /// Shift left / right
    ShiftLeft, ShiftRight,
    /// Comparison operator
    Equal, NotEqual, Greater, GreaterEq, Lesser, LesserEq
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum FuncKind {
    Log2, Log10, Power, Round, Ceil, Floor,
}

impl std::fmt::Display for FuncKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use FuncKind::*;
        match &self {
            Log2  =>  write!(f, "log2"),
            Log10 =>  write!(f, "log10"),
            Power =>  write!(f, "pow"),
            Round =>  write!(f, "round"),
            Ceil  =>  write!(f, "ceil"),
            Floor =>  write!(f, "floor"),
        }
    }
}


#[derive(Clone, PartialEq, Debug)]
pub enum Token {
    /// Basic math operator: +,-,*,/,%,^
    Operator(OpKind),
    /// Function call: ceil, log2
    FuncCall(FuncKind),
    /// Left Parenthesis
    ParenL,
    /// Right Parenthesis
    ParenR,
    /// Comma (used as argument separator in function call)
    Comma,
    /// Number
    Number(f64),
    /// Variable (starts with $)
    Var(String),
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Token::*;
        use OpKind::*;
        match &self {
            Operator(Plus)  => write!(f, "+"),
            Operator(Minus) => write!(f, "-"),
            Operator(Mult)  => write!(f, "*"),
            Operator(Div)   => write!(f, "/"),
            Operator(Pow)   => write!(f, "^"),
            Operator(Rem)   => write!(f, "%"),
            Operator(Equal)     => write!(f, "=="),
            Operator(NotEqual)  => write!(f, "!="),
            Operator(Greater)   => write!(f, ">"),
            Operator(GreaterEq) => write!(f, ">="),
            Operator(Lesser)    => write!(f, "<"),
            Operator(LesserEq)  => write!(f, "<="),
            Operator(ShiftLeft)  => write!(f, "<<"),
            Operator(ShiftRight) => write!(f, ">>"),
            Operator(Not) => write!(f, "!"),
            FuncCall(s) => write!(f, "{s}()"),
            ParenL     => write!(f, "("),
            ParenR     => write!(f, ")"),
            Comma      => write!(f, ","),
            Number(v)  => write!(f, "{v}"),
            Var(n)     => write!(f, "${n}"),
        }
    }
}

fn operator<'a>(input: &mut &'a str) -> Res<'a, Token> {
    use Token::Operator;
    use OpKind::*;
    alt((
        ws("+").value(Operator(Plus)),
        ws("-").value(Operator(Minus)),
        ws("*").value(Operator(Mult)),
        ws("/").value(Operator(Div)),
        ws("^").value(Operator(Pow)),
        ws("%").value(Operator(Rem)),
        ws("==").value(Operator(Equal)),
        ws("!=").value(Operator(NotEqual)),
        ws(">").value(Operator(Greater)),
        ws(">=").value(Operator(GreaterEq)),
        ws("<").value(Operator(Lesser)),
        ws("<=").value(Operator(LesserEq)),
        ws("<<").value(Operator(ShiftLeft)),
        ws(">>").value(Operator(ShiftRight)),
    )).parse_next(input)
}

fn not<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(alt(("not","!","~"))).value(Token::Operator(OpKind::Not)).parse_next(input)
}

fn parenl<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws("(").value(Token::ParenL).parse_next(input)
}

fn parenr<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(")").value(Token::ParenR).parse_next(input)
}

fn comma<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(",").value(Token::Comma).parse_next(input)
}

fn func_call<'a>(input: &mut &'a str) -> Res<'a, Token> {
    use Token::FuncCall;
    use FuncKind::*;
    alt((
        ws("log2(").value(FuncCall(Log2)),
        ws("log10(").value(FuncCall(Log10)),
        ws("pow(").value(FuncCall(Power)),
        ws("int(").value(FuncCall(Round)),
        ws("round(").value(FuncCall(Round)),
        ws("ceil(").value(FuncCall(Ceil)),
        ws("floor(").value(FuncCall(Floor)),
    )).parse_next(input)
}

fn variable<'a>(input: &mut &'a str) -> Res<'a, Token> {
    delimited("$", identifier, space0).map(|n| Token::Var(n.to_owned())).parse_next(input)
}

fn idx<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws("i").value(Token::Var("i".to_owned())).parse_next(input)
}

fn number<'a>(input: &mut &'a str) -> Res<'a, Token> {
    ws(alt((
        val_isize.map(|v| Token::Number(v as f64)),
        val_f64.map(Token::Number),
        Caseless("true").value(Token::Number(1.0)),
        Caseless("false").value(Token::Number(0.0)),
    ))).parse_next(input)
}

fn precedence(op: OpKind) -> u8 {
    match op {
        //
        OpKind::Not => 2,
        // Multiplication, Division Remainder
        OpKind::Mult => 3,
        OpKind::Div => 3,
        OpKind::Rem => 3,
        // Addition/Subtraction
        OpKind::Plus => 4,
        OpKind::Minus => 4,
        // Power
        OpKind::Pow => 5,
        // Shift
        OpKind::ShiftLeft => 5,
        OpKind::ShiftRight => 5,
        // Comparison
        OpKind::Equal => 7,
        OpKind::NotEqual => 7,
        OpKind::Greater => 6,
        OpKind::GreaterEq => 6,
        OpKind::Lesser => 6,
        OpKind::LesserEq => 6,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ExprState {
    Operand,
    Operator
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ExprContext {
    SubExpr,
    FuncCall(u8)
}


#[allow(dead_code)]
/// Parse a string and return a sequence of tokens in Reverse-Polish Notation (RPN)
/// Use the Shunting Yard Algorithm to transform infix notation to RPN:
/// 1.  While there are tokens to be read:
/// 2.        Read a token
/// 3.        If it's a number add it to queue
/// 4.        If it's an operator
/// 5.               While there's an operator on the top of the stack with greater precedence:
/// 6.                       Pop operators from the stack onto the output queue
/// 7.               Push the current operator onto the stack
/// 8.        If it's a left bracket push it onto the stack
/// 9.        If it's a right bracket
/// 10.            While there's not a left bracket at the top of the stack:
/// 11.                     Pop operators from the stack onto the output queue.
/// 12.             Pop the left bracket from the stack and discard it
/// 13. While there are operators on the stack, pop them to the queue
pub fn parse_expr(input: &str) -> Result<ExprTokens,RifError> {
    let mut tokens = ExprTokens::new(2);
    //
    let mut op_stack = ExprTokens::new(1);
    let mut cntxt : Vec<ExprContext> = Vec::new();
    let mut state = ExprState::Operand;
    //
    let mut s = input;
    while !s.is_empty() {

        let token = match state {
            ExprState::Operand => alt((parenl,variable,idx,number,func_call, not)).parse_next(&mut s)?,
            ExprState::Operator => match cntxt.last() {
                None => operator(&mut s)?,
                Some(ExprContext::SubExpr) |
                Some(ExprContext::FuncCall(0)) => alt((operator,parenr)).parse_next(&mut s)?,
                Some(ExprContext::FuncCall(_)) => alt((operator,comma)).parse_next(&mut s)?,
            }
        };

        // println!("{state:?} : {token:?} s='{s}' | cntxt={cntxt:?} | Stack = {op_stack:?} | Output = {tokens:?}");
        match token {
            // Operand -> save token and change state to Operator
            Token::Number(_) |
            Token::Var(_) => {
                tokens.push(token);
                state = ExprState::Operator;
            },
            // Push not operator on stack
            Token::Operator(OpKind::Not) => op_stack.push(token),
            // Operator -> Move operator stack to output until higher precedence operator is found
            // and then push operator to the stack, and change state to operand
            Token::Operator(op_r) => {
                while let Some(t) = op_stack.last() {
                    match t {
                        Token::Operator(op_l) if precedence(op_r) >= precedence(*op_l) => tokens.push(op_stack.pop().unwrap()),
                        _ => break,
                    }
                }
                op_stack.push(token);
                state = ExprState::Operand;
            },
            // Function call: push on operator stack and increase parenthesis counter
            Token::FuncCall(kind) => {
                op_stack.push(token);
                let nb_sep = match kind {
                    FuncKind::Power => 1,
                    _ => 0,
                };
                cntxt.push(ExprContext::FuncCall(nb_sep));
            },
            // Open parenthesis: Push ParenL on operator stack
            Token::ParenL => {
                cntxt.push(ExprContext::SubExpr);
                op_stack.push(Token::ParenL);
            },
            // Closing parenthesis : Pop last context and pop operators stack
            Token::ParenR => {
                cntxt.pop();
                while let Some(op) = op_stack.pop() {
                    match op {
                        Token::ParenL => {
                            break;
                        },
                        Token::FuncCall(_) => {
                            tokens.push(op);
                            break
                        },
                        _ => {tokens.push(op)},
                    }
                }
            },
            // Argument separator : decrease the expected number of argument
            // and now expect operand
            Token::Comma => {
                state = ExprState::Operand;
                if let Some(ExprContext::FuncCall(n)) = cntxt.last_mut() {
                    *n -= 1;
                }
            }
        }
    }

    // println!("Done : {state:?} | cntxt={cntxt:?} | Stack = {op_stack:?} | Output = {tokens:?}");
    // Empty the operator stack once all tokens have been parsed
    while let Some(op) = op_stack.pop() {
        tokens.push(op);
    }

    Ok(tokens)
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ExprTokens(Vec<Token>);

impl Deref for ExprTokens {
    type Target = Vec<Token>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ExprTokens {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl ExprTokens {

    pub fn new(capacity: usize) -> Self {
        ExprTokens(Vec::with_capacity(capacity))
    }

    #[allow(dead_code)]
    pub fn eval(&self, variables: &ParamValues) -> Result<isize, ExprError> {
        if self.is_empty() {
            return Ok(0);
        }
        let mut values : Vec<f64> = Vec::with_capacity(self.len()>>1);
        // println!("[eval] Expression = {self:?}");
        for token in self.iter() {
            match token {
                Token::Number(v) => values.push(*v),
                Token::Var(n) => {
                    let v = variables.get(n).ok_or(ExprError::UnknownVar(n.to_owned()))?;
                    values.push(*v as f64)
                },
                Token::Operator(op) => {
                    let v2 = if *op != OpKind::Not {
                        values.pop().ok_or(ExprError::Malformed)?
                    } else {
                        0.0
                    };
                    let v1 = values.pop().ok_or(ExprError::Malformed)?;
                    let res =
                        match op {
                            OpKind::Plus  => v1+v2,
                            OpKind::Minus => v1-v2,
                            OpKind::Mult  => v1*v2,
                            OpKind::Div   => v1/v2,
                            OpKind::Rem   => (v1 as isize % v2 as isize) as f64,
                            OpKind::Pow   => v1.powf(v2),
                            // Logical inversion
                            OpKind::Not   => if v1==0.0 {1.0} else {0.0},
                            // Shift
                            OpKind::ShiftLeft  => ((v1 as isize) << v2 as usize) as f64,
                            OpKind::ShiftRight => ((v1 as isize) >> v2 as usize) as f64,
                            // Comparison
                            OpKind::Equal     => if v1 == v2 {1.0} else {0.0},
                            OpKind::NotEqual  => if v1 != v2 {1.0} else {0.0},
                            OpKind::Greater   => if v1 >  v2 {1.0} else {0.0},
                            OpKind::GreaterEq => if v1 >= v2 {1.0} else {0.0},
                            OpKind::Lesser    => if v1 <  v2 {1.0} else {0.0},
                            OpKind::LesserEq  => if v1 <= v2 {1.0} else {0.0},
                        };
                    // println!("[eval] {v1} {op:?} {v2} -> {res} | {values:?}");
                    values.push(res);
                },
                Token::FuncCall(func) => {
                    let v = values.pop().ok_or(ExprError::Malformed)?;
                    let res = match func {
                        FuncKind::Log2  => v.log2(),
                        FuncKind::Log10 => v.log10(),
                        FuncKind::Power   => {
                            let base = values.pop().ok_or(ExprError::Malformed)?;
                            base.powf(v)
                        },
                        FuncKind::Round => v.round(),
                        FuncKind::Ceil  => v.ceil(),
                        FuncKind::Floor => v.floor(),
                    };
                    values.push(res);
                },
                // Other token variant should never appear in the expression
                _ => return Err(ExprError::Malformed),
            }
        }
        // Cast result to integer and check the stack is empty at the end of the evaluation
        let result = values.pop().ok_or(ExprError::Malformed)?;
        if values.is_empty() {
            Ok(result.round() as isize)
        } else {
            Err(ExprError::Malformed)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprError {
    Malformed,
    UnknownVar(String),
}

impl From<ExprError> for String {
    fn from(value: ExprError) -> Self {
        match value {
            ExprError::Malformed => "Malformed expression".to_owned(),
            ExprError::UnknownVar(v) => format!("Unknown var {v} in expression"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParamValues(OrderDict<String,isize>);

impl ParamValues {

    pub fn new() -> Self {
        ParamValues(OrderDict::new())
    }

    pub fn new_with_idx(idx: isize) -> Self {
        let mut params = ParamValues(OrderDict::new());
        params.0.insert("i".to_owned(), idx);
        params
    }

    pub fn from_iter<'a, I>(dict: I) -> Result<Self,String>
    where I: Iterator<Item = (&'a String,&'a ExprTokens)> {
        let mut params = ParamValues(OrderDict::new());
        for (name,expr) in dict.into_iter() {
            let v = expr.eval(&params).map_err(|_| format!("Malformed parameter {name} : {expr:?}"))?;
            params.0.insert(name.to_owned(), v);
        }
        Ok(params)
    }

    pub fn compile<'a, I>(&mut self, dict: I) -> Result<(),String>
    where I: Iterator<Item = (&'a String,&'a ExprTokens)> {
        for (name,expr) in dict.into_iter() {
            if self.0.contains_key(name) {
                // println!("Skipping {name} = {expr:?} : current value is {:?}", self.0.get(name).unwrap());
                continue;
            }
            let v = expr.eval(self).map_err(|_| format!("Malformed parameter {name} : {expr:?}"))?;
            // println!("Compiling {name} = {expr:?} ==> {v}");
            self.0.insert(name.to_owned(), v);
        }
        Ok(())
    }

    pub fn get(&self, k: &String) -> Option<&isize> {
        self.0.get(k)
    }

    pub fn insert(&mut self, k: String, v: isize) {
        self.0.insert(k,v);
    }

    pub fn items(&self) -> impl Iterator<Item=(&String,&isize)> {
        self.0.items()
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.0.len()==0
    }
}

impl Display for ParamValues {

    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tab = if f.alternate() {"\t"} else {""};
        let end = if f.alternate() {"\n"} else {", "};
        if f.alternate() {
            writeln!(f)?;
        }
        for (k,v) in self.items() {
            write!(f, "{tab}{k} = {v}{end}")?;
        }
        Ok(())
    }
}



#[cfg(test)]
mod tests_parsing {
    use super::*;
    use super::OpKind::*;
    use super::FuncKind::*;
    use super::Token::*;

    #[test]
    fn test_parse_expr() {
        assert_eq!(
            parse_expr(&mut "256 "),
            Ok(ExprTokens(vec![Number(256.0)]))
        );

        assert_eq!(
            parse_expr(&mut "$v1 +3"),
            Ok(ExprTokens(vec![Var("v1".to_owned()), Number(3.0), Operator(Plus)]))
        );

        assert_eq!(
            parse_expr(&mut "ceil(log2($v3-5))"),
            Ok(ExprTokens(vec![Var("v3".to_owned()), Number(5.0), Operator(Minus), FuncCall(Log2), FuncCall(Ceil)]))
        );

        assert_eq!(
            parse_expr(&mut "pow(3,$x )-1"),
            Ok(ExprTokens(vec![Number(3.0), Var("x".to_owned()), FuncCall(Power), Number(1.0), Operator(Minus)]))
        );
    }

    #[test]
    fn test_eval_expr() {
        let mut variables = ParamValues(OrderDict::new());
        variables.0.insert("v1".to_owned(), 1);
        variables.0.insert("x".to_owned(), 17);
        let expr = parse_expr(&mut "16*(not $v1) + 256*$v1").unwrap();
        assert_eq!(expr.eval(&variables),Ok(256));
        let expr = parse_expr(&mut "pow(2, $x) - 1").unwrap();
        assert_eq!(expr.eval(&variables),Ok((1<<17)-1));
    }

}