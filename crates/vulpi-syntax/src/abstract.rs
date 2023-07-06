use std::ops::Range;

use vulpi_location::{Byte, Spanned};
use vulpi_macros::Tree;
use vulpi_storage::interner::Symbol;
use vulpi_tree::{Show, TreeDisplay};

#[derive(Debug)]
pub struct Ident(pub Symbol);

impl Show for Ident {
    fn show(&self) -> vulpi_tree::TreeDisplay {
        TreeDisplay::label(&self.0.get())
    }
}

#[derive(Debug, Tree)]
pub struct Path {
    pub segments: Vec<Ident>,
    pub last: Ident,
    pub span: Range<Byte>,
}

#[derive(Default, Tree, Debug)]
pub struct Effects {
    pub effects: Vec<Type>,
}

#[derive(Tree, Debug)]
pub struct TypeArrow {
    pub left: Box<Type>,
    pub effects: Effects,
    pub right: Box<Type>,
}

#[derive(Tree, Debug)]
pub struct TypeApplication {
    pub left: Box<Type>,
    pub right: Vec<Type>,
}

#[derive(Tree, Debug)]
pub struct TypeForall {
    pub params: Vec<Ident>,
    pub body: Box<Type>,
}

#[derive(Tree, Debug)]
pub enum TypeKind {
    Upper(Path),
    Lower(Ident),
    Arrow(TypeArrow),
    Application(TypeApplication),
    Forall(TypeForall),
    Unit,
}

pub type Type = Spanned<TypeKind>;

// Literal

#[derive(Tree, Debug)]
pub enum LiteralKind {
    String(Ident),
    Integer(Ident),
    Char(Ident),
    Float(Ident),
    Unit,
}

pub type Literal = Spanned<LiteralKind>;

// Pattern

#[derive(Tree, Debug)]
pub struct PatAnnotation {
    pub pat: Box<Pattern>,
    pub ty: Type,
}

#[derive(Tree, Debug)]
pub struct PatOr {
    pub left: Box<Pattern>,
    pub right: Box<Pattern>,
}

#[derive(Tree, Debug)]
pub struct PatApplication {
    pub func: Path,
    pub args: Vec<Pattern>,
}

#[derive(Tree, Debug)]
pub enum PatternKind {
    Wildcard,
    Upper(Path),
    Lower(Ident),
    Literal(Literal),
    Annotation(PatAnnotation),
    Or(PatOr),
    Application(PatApplication),
}

pub type Pattern = Spanned<PatternKind>;

// Expression

#[derive(Tree, Debug)]
pub struct LetSttm {
    pub name: Box<Pattern>,
    pub expr: Box<Expr>,
}

#[derive(Tree, Debug)]
pub enum Statement {
    Let(LetSttm),
    Expr(Expr),
}

#[derive(Tree, Debug)]
pub struct Block {
    pub statements: Vec<Statement>,
}

#[derive(Tree, Debug)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
    Not,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    Shl,
    Shr,
    Pipe,
}

#[derive(Tree, Debug)]
pub struct LambdaExpr {
    pub pattern: Vec<Pattern>,
    pub body: Box<Expr>,
}

#[derive(Tree, Debug)]
pub struct ApplicationExpr {
    pub func: Box<Expr>,
    pub args: Vec<Expr>,
}

#[derive(Tree, Debug)]
pub struct AcessorExpr {
    pub expr: Box<Expr>,
    pub field: Ident,
}

#[derive(Tree, Debug)]
pub struct BinaryExpr {
    pub left: Box<Expr>,
    pub op: Operator,
    pub right: Box<Expr>,
}

#[derive(Tree, Debug)]
pub struct IfExpr {
    pub cond: Box<Expr>,
    pub then: Box<Expr>,
    pub else_: Box<Expr>,
}

#[derive(Tree, Debug)]
pub struct WhenArm {
    pub pattern: Box<Pattern>,
    pub then: Box<Expr>,
}

#[derive(Tree, Debug)]
pub struct WhenExpr {
    pub scrutinee: Box<Expr>,
    pub arms: Vec<WhenArm>,
}

#[derive(Tree, Debug)]
pub struct AnnotationExpr {
    pub expr: Box<Expr>,
    pub ty: Box<Type>,
}

#[derive(Tree, Debug)]
pub struct LetExpr {
    pub name: Box<Pattern>,
    pub value: Box<Expr>,
    pub body: Box<Expr>,
}

#[derive(Tree, Debug)]
pub enum ExprKind {
    Lambda(LambdaExpr),
    Application(ApplicationExpr),
    Ident(Path),
    Acessor(AcessorExpr),
    Binary(BinaryExpr),
    Let(LetExpr),
    If(IfExpr),
    When(WhenExpr),
    Annotation(AnnotationExpr),
    Block(Block),
    Literal(Literal),
}

pub type Expr = Spanned<ExprKind>;

// Top level

#[derive(Tree, Debug)]
pub struct LetCase {
    pub patterns: Vec<(Pattern, Type)>,
    pub body: Box<Expr>,
}

#[derive(Tree, Debug)]
pub struct LetDecl {
    pub name: Ident,
    pub cases: Vec<LetCase>,
}

#[derive(Tree, Debug)]
pub struct Constructor {
    pub name: Ident,
    pub args: Vec<Type>,
}

#[derive(Tree, Debug)]
pub struct SumDecl {
    pub constructors: Vec<Constructor>,
}

#[derive(Tree, Debug)]
pub struct Field {
    pub name: Ident,
    pub ty: Box<Type>,
}

#[derive(Tree, Debug)]
pub struct RecordDecl {
    pub fields: Vec<Field>,
}

#[derive(Tree, Debug)]
pub enum TypeDef {
    Sum(SumDecl),
    Record(RecordDecl),
    Synonym(Type),
}

#[derive(Tree, Debug)]
pub struct TypeDecl {
    pub name: Ident,
    pub params: Vec<Ident>,
    pub def: TypeDef,
}

#[derive(Tree, Debug)]
pub struct UseDecl {
    pub path: Path,
    pub alias: Option<Path>,
}

#[derive(Tree, Debug)]
pub struct Program {
    pub uses: Vec<UseDecl>,
    pub types: Vec<TypeDecl>,
    pub lets: Vec<LetDecl>,
}
