use vulpi_location::Spanned;
use vulpi_macros::Show;

use crate::tokens::Token;

use super::{literal::Literal, r#type::Type, Lower, Parenthesis, Path, Upper};

#[derive(Show, Clone)]
pub struct PatAscription {
    pub left: Box<Pattern>,
    pub colon: Token,
    pub right: Box<Type>,
}

#[derive(Show, Clone)]
pub struct PatApplication {
    pub func: Path<Upper>,
    pub args: Vec<Box<Pattern>>,
}

#[derive(Show, Clone)]
pub enum PatternKind {
    Wildcard(Token),
    Constructor(Path<Upper>),
    Variable(Lower),
    Literal(Literal),
    Annotation(PatAscription),
    Tuple(Vec<(Pattern, Option<Token>)>),
    Application(PatApplication),
    Parenthesis(Parenthesis<Box<Pattern>>),
}

pub type Pattern = Spanned<PatternKind>;
