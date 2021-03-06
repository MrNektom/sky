pub mod ast;

pub(crate) mod lexer;

pub(crate) mod scope;
pub(crate) mod symbols;
pub(crate) mod types;

use std::{collections::HashMap, usize};

use crate::{
    error::{Error, ErrorKind},
    parser::{
        ast::{BinOp, BinOpKind, Call, Expr, IfExpr, NumExpr, VarDefExpr},
        lexer::{Lexer, LitKind, Token, TokenKind},
    },
};

use self::{
    ast::FnExpr,
    scope::Scope,
    symbols::{Symbol, UnkownSymbol},
    types::Type,
};

pub struct Parser<'a> {
    lexer: Lexer<'a>,
    pub errors: Vec<Error>,
    code: &'a str,
    scope_stack: Vec<Scope>,
}

impl<'a> Parser<'a> {
    pub fn new(code: &'a str) -> Self {
        Self {
            lexer: Lexer::new(code),
            errors: Vec::new(),
            code,
            scope_stack: Vec::new(),
        }
    }
    pub fn parse_top(&mut self) -> Option<Expr> {
        let mut exprs = Vec::new();
        self.scope_stack.push(Scope::new_named("global"));
        while !self.lexer.eof() {
            let expr = self.parse_expr();
            if expr.is_some() {
                exprs.push(expr.unwrap());
                if self.has_str(";") {
                    self.lexer.next();
                }
            } else {
                break;
            }
        }
        if exprs.len() == 1 {
            exprs.pop()
        } else {
            Some(Expr::CodeBlock(exprs))
        }
    }
    fn parse_expr(&mut self) -> Option<Expr> {
        let mut expr = self.parse_atom()?;
        expr = self.maybe_call(expr);
        expr = self.maybe_binary(expr);
        Some(expr)
    }
    fn parse_atom(&mut self) -> Option<Expr> {
        self.skip_whitespace();
        let tok = self.lexer.peek()?;
        match tok.kind {
            TokenKind::Lit { kind } => match kind {
                LitKind::Num { .. } => self.parse_num(),
                LitKind::Str => self.parse_str(),
            },
            TokenKind::OpenParen => self.parse_tuple(),
            TokenKind::OpenBrace => self.parse_block(),
            TokenKind::Ident => self.parse_ident(),
            _ => {
                self.push_error(ErrorKind::UnexpectedToken, tok.index, tok.size);
                None
            }
        }
    }
    fn parse_ident(&mut self) -> Option<Expr> {
        match self.lexer.get_tok()? {
            "if" => self.parse_if(),
            "let" => self.parse_let(),
            "fn" => self.parse_fn(),
            "null" => Some(Expr::Null),
            _ => self.parse_sym(),
        }
    }
    fn parse_if(&mut self) -> Option<Expr> {
        self.lexer.next();
        let cond = self.parse_expr()?;
        let then_branch = self.parse_expr()?;
        let mut else_branch = None;
        self.skip_whitespace();
        if self.has_str("else") {
            self.lexer.next();
            else_branch = self.parse_expr();
        }
        Some(Expr::If(Box::new(IfExpr {
            cond,
            then_branch,
            else_branch,
        })))
    }
    fn parse_let(&mut self) -> Option<Expr> {
        if !self.has_str("let") {
            return None;
        }
        self.lexer.next();
        self.skip_whitespace();
        let mut is_mut = false;
        let mut name = String::new();
        let mut initial = None;
        if self.has_str("mut") {
            is_mut = true;
            self.lexer.next();
        }
        self.skip_whitespace();
        if self.has_type(TokenKind::Ident) {
            name.push_str(self.lexer.get_tok()?);
            self.lexer.next();
        } else {
            is_mut = false;
            name.push_str("mut");
        }
        self.skip_whitespace();
        dbg!(self.lexer.peek());

        if self.has_type(TokenKind::Eq) {
            self.lexer.next();
            self.skip_whitespace();
            initial = Some(Box::new(self.parse_expr()?));
        }
        Some(Expr::VarDef(Box::new(VarDefExpr {
            name,
            is_mut,
            initial,
        })))
    }

    fn parse_fn(&mut self) -> Option<Expr> {
        let mut name = "<anonymous>";
        let mut args: HashMap<String, Type> = HashMap::new();
        let mut ret = Expr::Null;
        if self._get_tok_val(self.lexer.peek()?)? == "fn" {
            self.lexer.next();
        }
        let mut tok = self.lexer.peek()?;
        if tok.kind == TokenKind::Ident {
            name = self._get_tok_val(tok)?;
        }

        Some(Expr::Fn(FnExpr {
            name: name.to_string(),
            args,
            ret: Box::new(ret),
        }))
    }

    fn parse_block(&mut self) -> Option<Expr> {
        if !self.has_str("{") {
            return None;
        }
        self.scope_stack
            .push(self.scope_stack.last().unwrap().child());
        self.lexer.next();
        let mut buff = Vec::new();
        while !self.has_str("}") {
            buff.push(self.parse_expr()?);
            if self.has_str(";") {
                self.lexer.next();
            }
            self.skip_whitespace();
            if self.lexer.get_str(1) == Some("}") {
                self.lexer.next();
                break;
            }
        }
        Some(Expr::CodeBlock(buff))
    }
    fn parse_num(&mut self) -> Option<Expr> {
        if let Some(Token {
            kind:
                TokenKind::Lit {
                    kind: LitKind::Num { base, suff_off },
                },
            size,
            index,
        }) = self.lexer.next()
        {
            let mut start = index;
            let mut end = index + size;
            let mut radix = 10;
            if let Some(base) = base {
                start += 2;
                radix = base.into();
            }
            let suff = match suff_off {
                Some(offset) => {
                    end = index + offset;
                    self.code.get(index + offset..index + size)
                }
                None => None,
            };
            let mut val = self.code.get(start..end)?;
            let base = match base {
                None => 10,
                Some(b) => b.into(),
            };
            let expr = Expr::Num(match suff {
                Some("i32") => {
                    if val.contains('.') {
                        val = val.get(..val.find('.')?)?;
                    }
                    NumExpr::I32(i32::from_str_radix(val, radix).ok()?)
                }
                Some("i64") => {
                    if val.contains('.') {
                        val = val.get(..val.find('.')?)?;
                    }
                    NumExpr::I64(i64::from_str_radix(val, radix).ok()?)
                }
                Some("u32") => {
                    if val.contains('.') {
                        val = val.get(..val.find('.')?)?;
                    }
                    NumExpr::U32(u32::from_str_radix(val, radix).ok()?)
                }
                Some("u64") => {
                    if val.contains('.') {
                        val = val.get(..val.find('.')?)?;
                    }
                    NumExpr::U64(u64::from_str_radix(val, radix).ok()?)
                }
                Some("f32") => NumExpr::F32(parse_based_f32(base, val)?),
                Some("f64") => NumExpr::F64(parse_based_f64(base, val)?),
                None if val.contains('.') => NumExpr::F32(val.parse().ok()?),
                None => NumExpr::I32(i32::from_str_radix(val, radix).ok()?),
                Some(suff) => {
                    self.push_error(
                        ErrorKind::UnexpectedToken,
                        suff_off.unwrap_or(0),
                        suff.len(),
                    );
                    return None;
                }
            });
            Some(expr)
        } else {
            None
        }
    }

    fn parse_str(&mut self) -> Option<Expr> {
        if let Some(Token {
            kind: _,
            size,
            index,
        }) = self.lexer.next()
        {
            let string = self.code.get(index + 1..index + size - 1)?;

            Some(Expr::Str(escape_str(string)))
        } else {
            None
        }
    }

    fn maybe_call(&mut self, left: Expr) -> Expr {
        if self.has_str("(") {
            let args = self.parse_tuple();
            if args.is_some() {
                match args.unwrap() {
                    Expr::List(list) => Expr::Call(Box::new(Call {
                        args: list,
                        callee: left,
                    })),
                    _ => left,
                }
            } else {
                left
            }
        } else {
            left
        }
    }
    fn parse_tuple(&mut self) -> Option<Expr> {
        self.lexer.eat_whitespace();
        if self.has_str("(") {
            self.lexer.next();
            let mut list = Vec::new();
            while !self.has_str(")") {
                let expr = self.parse_expr()?;
                list.push(expr);
                if self.has_str(",") {
                    self.lexer.next();
                }
            }
            self.lexer.eat_whitespace();
            self.lexer.next();
            Some(Expr::List(list))
        } else {
            None
        }
    }
    fn maybe_binary(&mut self, left: Expr) -> Expr {
        self.skip_whitespace();
        if self.lexer.eof() {
            return left;
        }
        let Token {
            kind: _,
            size: _,
            index: _,
        } = self.lexer.peek().unwrap();
        if let Some(kind) = self.parse_bin_op() {
            let priory: u8 = kind.clone().into();
            let mut expr: Expr;
            let right = self.parse_expr();
            if right.is_none() {
                return left;
            }
            let right = right.unwrap();
            if let Expr::BinOp(right) = right {
                let r_priory: u8 = right.kind.clone().into();
                if priory >= r_priory {
                    expr = Expr::BinOp(Box::new(BinOp {
                        kind,
                        left,
                        right: right.left,
                    }));
                    expr = Expr::BinOp(Box::new(BinOp {
                        kind: right.kind,
                        left: expr,
                        right: right.right,
                    }));
                } else {
                    expr = Expr::BinOp(Box::new(BinOp {
                        kind: right.kind,
                        left: right.left,
                        right: right.right,
                    }));
                    expr = Expr::BinOp(Box::new(BinOp {
                        kind,
                        left,
                        right: expr,
                    }));
                }
            } else {
                expr = Expr::BinOp(Box::new(BinOp { kind, left, right }));
            }
            expr
        } else {
            left
        }
    }

    fn parse_bin_op(&mut self) -> Option<BinOpKind> {
        self.skip_whitespace();
        match self.lexer.peek()?.kind {
            TokenKind::Eq => {
                self.lexer.next();
                Some(match self.lexer.peek()?.kind {
                    TokenKind::Eq => {
                        self.lexer.next();
                        BinOpKind::Eq
                    }
                    _ => BinOpKind::Assign,
                })
            }
            TokenKind::Lt => {
                self.lexer.next();
                Some(match self.lexer.peek()?.kind {
                    TokenKind::Eq => {
                        self.lexer.next();
                        BinOpKind::LtEq
                    }
                    _ => BinOpKind::Lt,
                })
            }
            TokenKind::Gt => {
                self.lexer.next();
                Some(match self.lexer.peek()?.kind {
                    TokenKind::Eq => {
                        self.lexer.next();
                        BinOpKind::GtEq
                    }
                    _ => BinOpKind::Gt,
                })
            }
            TokenKind::Add => {
                self.lexer.next();
                Some(BinOpKind::Add)
            }
            TokenKind::Sub => {
                self.lexer.next();
                Some(BinOpKind::Sub)
            }
            TokenKind::Mul => {
                self.lexer.next();
                Some(match self.lexer.peek()?.kind {
                    TokenKind::Mul => {
                        self.lexer.next();
                        BinOpKind::Pow
                    }
                    _ => BinOpKind::Mul,
                })
            }
            TokenKind::Div => Some(BinOpKind::Div),
            TokenKind::Percent => Some(BinOpKind::Mod),
            _ => None,
        }
    }

    fn parse_sym(&mut self) -> Option<Expr> {
        let tok = self.lexer.peek()?;
        match tok.kind {
            TokenKind::Ident => {
                let sym = Expr::Symbol(Symbol::Unkown(UnkownSymbol {
                    name: self._get_tok_val(tok)?.to_owned(),
                    line: 0,
                    col: 0,
                }));
                self.lexer.next();
                self.lexer.eat_whitespace();
                let tok = self.lexer.peek();
                match tok {
                    Some(tok) => match tok.kind {
                        TokenKind::Colon => {
                            self.lexer.next();
                            let right = self.parse_sym();
                            if right.is_some() {
                                Some(Expr::NSAccess(Box::new(sym), Box::new(right?)))
                            } else {
                                Some(sym)
                            }
                        }
                        _ => Some(sym),
                    },
                    _ => Some(sym),
                }
            }
            _ => None,
        }
    }

    fn push_error(&mut self, kind: ErrorKind, index: usize, len: usize) {
        dbg!(self.get_str(index, len));
        self.errors.push(Error::new(kind, index, len));
    }
    fn skip_whitespace(&mut self) {
        if let Some(Token {
            kind: TokenKind::Whitespace,
            ..
        }) = self.lexer.peek()
        {
            self.lexer.next();
        }
    }
    // fn has(&mut self, token_type: TokenKind) -> bool {
    //     match self.lexer.peek() {
    //         None => false,
    //         Some(Token { kind, .. }) => kind == token_type,
    //     }
    // }
    fn has_str(&self, s: &str) -> bool {
        let ss = self.lexer.get_str(s.len());
        if Some(s) == ss {
            true
        } else {
            false
        }
    }
    fn has_type(&self, kind: TokenKind) -> bool {
        let tok = self.lexer.peek();
        match tok {
            Some(tok) => tok.kind == kind,
            None => false,
        }
    }
    pub fn _get_tok_val(&self, tok: Token) -> Option<&str> {
        self.code.get(tok.index..(tok.index + tok.size))
    }
    pub fn get_str(&self, index: usize, len: usize) -> Option<&str> {
        self.code.get(index..(index + len))
    }
    fn scope(&self) -> Scope {
        self.scope_stack.last().unwrap().clone()
    }
}

fn escape_str(src: &str) -> String {
    let mut buf = String::new();
    iterate_str(src, |one, two| match one {
        Some('\\') => match two {
            Some('n') => buf.push('\n'),
            Some('r') => buf.push('\r'),
            Some('t') => buf.push('\t'),
            Some('\\') => buf.push('\\'),
            _ => (),
        },
        Some(ch) => buf.push(ch),
        None => (),
    });
    buf
}

fn iterate_str<CB>(s: &str, mut call_back: CB)
where
    CB: FnMut(Option<char>, Option<char>),
{
    let mut chars = s.chars();
    let mut one = chars.next();
    let mut two = chars.next();
    while one.is_some() {
        call_back(one, two);
        one = chars.next();
        two = chars.next();
    }
}
pub fn parse_based_f64(base: u32, num: &str) -> Option<f64> {
    let mut left: f64;
    let mut divider = 1f64;
    let mut right: f64;
    if num.contains('.') {
        let mut s = num.split('.');
        left = i32::from_str_radix(s.next()?, base).ok()? as f64;
        right = i32::from_str_radix(s.next()?, base).ok()? as f64;
        while divider < right {
            divider *= 10f64;
        }
        right /= divider;
        left += right;
    } else {
        left = i32::from_str_radix(num, base).ok()? as f64;
    }
    Some(left)
}

pub fn parse_based_f32(base: u32, num: &str) -> Option<f32> {
    let mut left: f32;
    let mut divider = 1f32;
    let mut right: f32;
    if num.contains('.') {
        let mut s = num.split('.');
        left = i32::from_str_radix(s.next()?, base).ok()? as f32;
        right = i32::from_str_radix(s.next()?, base).ok()? as f32;
        while divider < right {
            divider *= 10f32;
        }
        right /= divider;
        left += right;
    } else {
        left = i32::from_str_radix(num, base).ok()? as f32;
    }
    Some(left)
}
