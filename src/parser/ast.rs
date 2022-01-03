#[derive(Debug, Clone)]
pub enum Expr {
    Num(String),
    Str(String),
    Access(Box<Expr>, Box<Expr>),
    BinOp(String, Box<Expr>, Box<Expr>),
    CodeBlock(Vec<Expr>),
    Closure(Vec<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    List(Vec<Expr>),
    Null,
}

impl Expr {
    pub(crate) fn is_num(&self) -> bool {
        matches!(*self, Expr::Num(_))
    }
    pub(crate) fn is_str(&self) -> bool {
        matches!(*self, Expr::Str(_))
    }
    pub(crate) fn is_access(&self) -> bool {
        matches!(*self, Expr::Access(_, _))
    }
    pub(crate) fn is_bin_op(&self) -> bool {
        matches!(*self, Expr::BinOp(_, _, _))
    }
    pub(crate) fn is_code_block(&self) -> bool {
        matches!(*self, Expr::CodeBlock(_))
    }
    pub(crate) fn is_closure(&self) -> bool {
        matches!(*self, Expr::Closure(_, _))
    }
    pub(crate) fn is_if(&self) -> bool {
        matches!(*self, Expr::If(_, _, _))
    }
    pub(crate) fn is_null(&self) -> bool {
        matches!(*self, Expr::Null)
    }

    pub(crate) fn as_bin_op(self) -> Option<(String, Box<Expr>, Box<Expr>)> {
        match self {
            Expr::BinOp(op, left, right) => Some((op, left, right)),
            _ => None,
        }
    }
}
