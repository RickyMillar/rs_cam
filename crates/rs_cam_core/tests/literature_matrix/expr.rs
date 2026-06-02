//! Closed-form expression evaluator for literature_matrix invariants.
//!
//! f64-only. Supports numeric literals, identifiers, parentheses, unary
//! minus, binary arithmetic (`+ - * /`), comparisons (`< <= > >= == !=`),
//! and logical combinators (`&& ||`). Booleans encode as 1.0 / 0.0.
//!
//! No functions, no strings, no side effects. Identifiers not in the
//! `Bindings` map produce `EvalError::UnknownVar`.
//!
//! Used by `invariant.rs` to evaluate cell `expr =` strings against the
//! shim's engine-output snapshot.

use std::collections::HashMap;
use std::fmt;

pub type Bindings = HashMap<String, f64>;

#[derive(Debug, Clone, PartialEq)]
pub enum EvalError {
    UnknownVar(String),
    DivByZero,
    ParseError(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EvalError::UnknownVar(v) => write!(f, "unknown variable: {v}"),
            EvalError::DivByZero => write!(f, "division by zero"),
            EvalError::ParseError(s) => write!(f, "parse error: {s}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Lt,
    Le,
    Gt,
    Ge,
    EqEq,
    Ne,
    AndAnd,
    OrOr,
    LParen,
    RParen,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, EvalError> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }
        match b {
            b'+' => {
                out.push(Tok::Plus);
                i += 1;
            }
            b'-' => {
                out.push(Tok::Minus);
                i += 1;
            }
            b'*' => {
                out.push(Tok::Star);
                i += 1;
            }
            b'/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            b'(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            b')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            b'<' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    out.push(Tok::Le);
                    i += 2;
                } else {
                    out.push(Tok::Lt);
                    i += 1;
                }
            }
            b'>' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    out.push(Tok::Ge);
                    i += 2;
                } else {
                    out.push(Tok::Gt);
                    i += 1;
                }
            }
            b'=' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    out.push(Tok::EqEq);
                    i += 2;
                } else {
                    return Err(EvalError::ParseError("bare '=' is not supported".into()));
                }
            }
            b'!' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'=' {
                    out.push(Tok::Ne);
                    i += 2;
                } else {
                    return Err(EvalError::ParseError("bare '!' is not supported".into()));
                }
            }
            b'&' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'&' {
                    out.push(Tok::AndAnd);
                    i += 2;
                } else {
                    return Err(EvalError::ParseError("bare '&' is not supported".into()));
                }
            }
            b'|' => {
                if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
                    out.push(Tok::OrOr);
                    i += 2;
                } else {
                    return Err(EvalError::ParseError("bare '|' is not supported".into()));
                }
            }
            _ if b.is_ascii_digit() || b == b'.' => {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_digit() || bytes[i] == b'.' || bytes[i] == b'_')
                {
                    i += 1;
                }
                // optional exponent
                if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                    i += 1;
                    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
                        i += 1;
                    }
                    while i < bytes.len() && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let lit: String = src[start..i].chars().filter(|c| *c != '_').collect();
                let n: f64 = lit
                    .parse()
                    .map_err(|e| EvalError::ParseError(format!("bad number `{lit}`: {e}")))?;
                out.push(Tok::Num(n));
            }
            _ if b.is_ascii_alphabetic() || b == b'_' => {
                let start = i;
                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_')
                {
                    i += 1;
                }
                let ident = &src[start..i];
                out.push(Tok::Ident(ident.to_owned()));
            }
            _ => {
                return Err(EvalError::ParseError(format!(
                    "unexpected character `{}` at byte {}",
                    b as char, i
                )));
            }
        }
    }
    Ok(out)
}

// Pratt parser with explicit precedence.
//   ||  -> 1
//   &&  -> 2
//   == !=         -> 3
//   < <= > >=     -> 4
//   + -           -> 5
//   * /           -> 6
//   unary -       -> 7
fn precedence(tok: &Tok) -> Option<u8> {
    match tok {
        Tok::OrOr => Some(1),
        Tok::AndAnd => Some(2),
        Tok::EqEq | Tok::Ne => Some(3),
        Tok::Lt | Tok::Le | Tok::Gt | Tok::Ge => Some(4),
        Tok::Plus | Tok::Minus => Some(5),
        Tok::Star | Tok::Slash => Some(6),
        _ => None,
    }
}

#[derive(Debug, Clone)]
enum Expr {
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy)]
enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
}

fn bin_op_of(tok: &Tok) -> Option<BinOp> {
    Some(match tok {
        Tok::Plus => BinOp::Add,
        Tok::Minus => BinOp::Sub,
        Tok::Star => BinOp::Mul,
        Tok::Slash => BinOp::Div,
        Tok::Lt => BinOp::Lt,
        Tok::Le => BinOp::Le,
        Tok::Gt => BinOp::Gt,
        Tok::Ge => BinOp::Ge,
        Tok::EqEq => BinOp::Eq,
        Tok::Ne => BinOp::Ne,
        Tok::AndAnd => BinOp::And,
        Tok::OrOr => BinOp::Or,
        _ => return None,
    })
}

struct Parser<'a> {
    toks: &'a [Tok],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_expr(&mut self, min_prec: u8) -> Result<Expr, EvalError> {
        let mut lhs = self.parse_atom()?;
        while let Some(tok) = self.peek() {
            let Some(p) = precedence(tok) else { break };
            if p < min_prec {
                break;
            }
            let op_tok = self
                .bump()
                .ok_or_else(|| EvalError::ParseError("unexpected end of input".into()))?;
            let op = bin_op_of(&op_tok).ok_or_else(|| {
                EvalError::ParseError(format!("expected binary operator, got {op_tok:?}"))
            })?;
            // left-associative: next operator must have STRICTLY higher precedence
            // to bind on the right.
            let rhs = self.parse_expr(p + 1)?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn parse_atom(&mut self) -> Result<Expr, EvalError> {
        let tok = self
            .bump()
            .ok_or_else(|| EvalError::ParseError("unexpected end of input".into()))?;
        match tok {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Ident(s) => Ok(Expr::Var(s)),
            Tok::Minus => {
                // unary minus binds tightly (precedence 7)
                let inner = self.parse_expr(7)?;
                Ok(Expr::Neg(Box::new(inner)))
            }
            Tok::LParen => {
                let inner = self.parse_expr(0)?;
                match self.bump() {
                    Some(Tok::RParen) => Ok(inner),
                    other => Err(EvalError::ParseError(format!(
                        "expected ')', got {other:?}"
                    ))),
                }
            }
            other => Err(EvalError::ParseError(format!(
                "unexpected token {other:?} in expression"
            ))),
        }
    }
}

fn eval_node(e: &Expr, b: &Bindings) -> Result<f64, EvalError> {
    match e {
        Expr::Num(n) => Ok(*n),
        Expr::Var(name) => b
            .get(name)
            .copied()
            .ok_or_else(|| EvalError::UnknownVar(name.clone())),
        Expr::Neg(inner) => Ok(-eval_node(inner, b)?),
        Expr::Bin(op, lhs, rhs) => {
            let l = eval_node(lhs, b)?;
            let r = eval_node(rhs, b)?;
            Ok(match op {
                BinOp::Add => l + r,
                BinOp::Sub => l - r,
                BinOp::Mul => l * r,
                BinOp::Div => {
                    if r == 0.0 {
                        return Err(EvalError::DivByZero);
                    }
                    l / r
                }
                BinOp::Lt => bool_to_f64(l < r),
                BinOp::Le => bool_to_f64(l <= r),
                BinOp::Gt => bool_to_f64(l > r),
                BinOp::Ge => bool_to_f64(l >= r),
                BinOp::Eq => bool_to_f64((l - r).abs() < f64::EPSILON),
                BinOp::Ne => bool_to_f64((l - r).abs() >= f64::EPSILON),
                BinOp::And => bool_to_f64(truthy(l) && truthy(r)),
                BinOp::Or => bool_to_f64(truthy(l) || truthy(r)),
            })
        }
    }
}

fn bool_to_f64(b: bool) -> f64 {
    if b { 1.0 } else { 0.0 }
}

fn truthy(v: f64) -> bool {
    v != 0.0 && !v.is_nan()
}

/// Evaluate `src` against `bindings`. Returns the numeric value; boolean
/// results encode as 1.0 (true) / 0.0 (false).
pub fn evaluate(src: &str, bindings: &Bindings) -> Result<f64, EvalError> {
    let toks = tokenize(src)?;
    if toks.is_empty() {
        return Err(EvalError::ParseError("empty expression".into()));
    }
    let mut p = Parser {
        toks: &toks,
        pos: 0,
    };
    let expr = p.parse_expr(0)?;
    if p.pos != toks.len() {
        return Err(EvalError::ParseError(format!(
            "unexpected trailing token at position {}",
            p.pos
        )));
    }
    eval_node(&expr, bindings)
}
