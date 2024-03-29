//! This is the module for the IR representation of the language. This is used to lower the AST into
//! a form that is easier to work with for code generation.

pub mod transform;
pub mod pattern;
pub mod inline;
pub mod dead_code;
pub mod uncurry;
