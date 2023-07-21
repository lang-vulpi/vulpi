use std::collections::HashSet;

use vulpi_macros::Tree;
use vulpi_report::{Diagnostic, Report};
use vulpi_storage::id::{self, Id};
use vulpi_storage::interner::Symbol;
use vulpi_syntax::r#abstract::*;

use crate::ambiguity::DataType;
use crate::error;

#[derive(Default, Tree)]
pub struct ModuleTree {
    name: Id<id::Namespace>,
    children: std::collections::HashMap<Symbol, ModuleTree>,
}

impl ModuleTree {
    pub fn new(name: Id<id::Namespace>) -> Self {
        Self {
            name,
            children: Default::default(),
        }
    }

    pub fn find(&self, path: &[Symbol]) -> Option<Id<id::Namespace>> {
        if path.is_empty() {
            return Some(self.name);
        }

        let first = path[0].clone();
        let tail = &path[1..];

        if let Some(child) = self.children.get(&first) {
            child.find(tail)
        } else {
            None
        }
    }

    pub fn find_sub_tree(&self, path: &[Symbol]) -> Option<&Self> {
        if path.is_empty() {
            return Some(self);
        }

        let first = path[0].clone();
        let tail = &path[1..];

        if let Some(child) = self.children.get(&first) {
            child.find_sub_tree(tail)
        } else {
            None
        }
    }

    /// Inserts a new module into the tree. If the module already exists, it returns the ID of the
    /// existing module.
    pub fn insert(&mut self, path: &[Symbol], id: Id<id::Namespace>) -> Option<Id<id::Namespace>> {
        if path.is_empty() {
            return Some(self.name);
        }

        let first = path[0].clone();
        let tail = &path[1..];

        if let Some(child) = self.children.get_mut(&first) {
            child.insert(tail, id)
        } else {
            self.children.insert(first, ModuleTree::new(id));
            None
        }
    }
}

#[derive(Tree)]
pub struct Definition {
    pub path: Vec<Symbol>,
    pub decls: HashSet<DataType>,
}

impl Definition {
    pub fn new(path: Vec<Symbol>) -> Self {
        Self {
            path,
            decls: Default::default(),
        }
    }
}

pub struct Modules {
    pub counter: usize,
    pub tree: ModuleTree,
    pub definitions: Vec<Definition>,
    pub current: Vec<Id<id::Namespace>>,
    pub module: Vec<Symbol>,
    pub reporter: Report,
    pub file_id: Id<id::File>,
}

impl Modules {
    pub fn new(reporter: Report, file_id: Id<id::File>) -> Self {
        Self {
            counter: 1,
            tree: ModuleTree::new(Id::new(0)),
            definitions: vec![Definition::new(vec![])],
            current: vec![Id::new(0)],
            module: Default::default(),
            reporter,
            file_id,
        }
    }

    pub fn find_module(&self, path: &[Symbol]) -> Option<Id<id::Namespace>> {
        self.tree.find(path)
    }

    pub fn add_module(&mut self, path: Vec<Symbol>) -> Option<Id<id::Namespace>> {
        let id = Id::new(self.counter);
        self.counter += 1;

        if self.tree.insert(&path, id).is_none() {
            self.module = path.clone();
        } else {
            self.counter -= 1;
            panic!("module already exists")
        }

        self.current.push(id);
        self.definitions.push(Definition::new(path));

        Some(id)
    }

    pub fn current(&mut self) -> &mut Definition {
        &mut self.definitions[self.current.last().unwrap().index()]
    }
}

pub trait Declare {
    fn declare(&mut self, context: &mut Modules);
}

impl<T: Declare> Declare for Vec<T> {
    fn declare(&mut self, context: &mut Modules) {
        for item in self {
            item.declare(context);
        }
    }
}

impl Declare for Variant {
    fn declare(&mut self, context: &mut Modules) {
        context
            .definitions
            .last_mut()
            .unwrap()
            .decls
            .insert(DataType::Constructor(self.name.0.clone()));
    }
}

impl Declare for TypeDecl {
    fn declare(&mut self, context: &mut Modules) {
        let defs = context.current();
        defs.decls.insert(DataType::Type(self.name.0.clone()));

        let old_path = context.module.clone();
        let mut path = context.module.clone();
        path.push(self.name.0.clone());

        let id = context.add_module(path).unwrap();

        self.id = Some(id);

        match &mut self.def {
            TypeDef::Enum(enum_) => enum_.variants.declare(context),
            TypeDef::Record(_) => (),
            TypeDef::Synonym(_) => (),
        }

        context.current.pop();
        context.module = old_path;
    }
}

impl Declare for LetDecl {
    fn declare(&mut self, context: &mut Modules) {
        let file = context.file_id;
        let reporter = context.reporter.clone();
        let defs = context.current();

        if defs.decls.contains(&DataType::Let(self.name.0.clone())) {
            reporter.report(Diagnostic::new(error::ResolverError {
                message: error::ResolverErrorKind::AlreadyCaptured(self.name.0.clone()),
                range: self.name.1.clone(),
                file,
            }))
        }

        defs.decls.insert(DataType::Let(self.name.0.clone()));
    }
}

impl Declare for Program {
    fn declare(&mut self, context: &mut Modules) {
        for type_decl in &mut self.types {
            type_decl.declare(context);
        }

        for let_decl in &mut self.lets {
            let_decl.declare(context);
        }
    }
}

pub fn declare(
    context: &mut Modules,
    program: &mut Program,
    path: Vec<Symbol>,
) -> Id<id::Namespace> {
    context.add_module(path).unwrap();
    program.declare(context);
    context.current.pop().unwrap()
}

pub fn declare_main(context: &mut Modules, program: &mut Program) -> Id<id::Namespace> {
    program.declare(context);
    context.current.pop().unwrap()
}
