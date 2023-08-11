//! This file declares a mutable environment that is useful to keep track of information that does
//! not need to be immutable like the Env.

use crate::{
    module::Modules,
    r#type::{eval::Eval, r#virtual::Pi, Index, State},
};
use im_rc::HashSet;
use vulpi_intern::Symbol;
use vulpi_report::{Diagnostic, Report};
use vulpi_syntax::{elaborated, r#abstract::Qualified};

use crate::{
    errors::{TypeError, TypeErrorKind},
    r#type::{
        eval::Quote,
        r#virtual::Env,
        r#virtual::Virtual,
        real::{self, Real},
        HoleInner, Level, Type, TypeKind,
    },
};

/// A mutable context that is used differently from [Env]. It is used to keep data between every
/// thing inside the type checker.
pub struct Context {
    pub counter: usize,
    pub reporter: Report,
    pub modules: Modules,
    pub elaborated: elaborated::Program,
}

impl Context {
    pub fn new(reporter: Report) -> Self {
        Self {
            counter: 0,
            reporter,
            modules: Default::default(),
            elaborated: Default::default(),
        }
    }

    pub fn report(&mut self, env: &Env, kind: TypeErrorKind) {
        self.reporter.report(Diagnostic::new(TypeError {
            span: env.span.borrow().clone(),
            kind,
        }));
    }

    fn inc_counter(&mut self) -> usize {
        self.counter += 1;
        self.counter - 1
    }

    /// Creates a new name with the prefix `t_` and a unique number.
    pub fn new_name(&mut self) -> Symbol {
        Symbol::intern(&format!("t_{}", self.inc_counter()))
    }

    /// Creates a new hole that is a type that is not yet known
    pub fn hole<S: State>(&mut self, env: &Env, kind: Type<S>) -> Type<S> {
        env.hole(kind, self.new_name())
    }

    /// Creates a "lacks" hole that stores effects that should lack.
    pub fn lacks(&mut self, env: &Env, hash_set: HashSet<Qualified>) -> Type<Virtual> {
        env.lacks(self.new_name(), hash_set)
    }

    pub fn as_function(
        &mut self,
        env: &Env,
        typ: Type<Virtual>,
    ) -> Option<(Type<Virtual>, Type<Virtual>, Type<Virtual>)> {
        match typ.deref().as_ref() {
            TypeKind::Arrow(pi) => Some((pi.ty.clone(), pi.effs.clone(), pi.body.clone())),
            TypeKind::Error => Some((typ.clone(), Type::new(TypeKind::Empty), typ.clone())),
            TypeKind::Forall(_) => {
                let typ = self.instantiate(env, &typ);
                self.as_function(env, typ)
            }
            TypeKind::Hole(empty) => {
                let hole_inner = empty.0.borrow().clone();
                if let HoleInner::Empty(_, kind, _) = hole_inner {
                    let hole_a = self.hole(env, kind.clone());
                    let hole_b = self.hole(env, kind);

                    empty.fill(Type::new(TypeKind::Arrow(Pi {
                        ty: hole_a.clone(),
                        effs: Type::new(TypeKind::Empty),
                        body: hole_b.clone(),
                    })));

                    Some((hole_a, Type::new(TypeKind::Empty), hole_b))
                } else {
                    unreachable!()
                }
            }
            _ => None,
        }
    }

    /// Instantiates a poly type to a monotype.
    pub fn instantiate(&mut self, env: &Env, ty: &Type<Virtual>) -> Type<Virtual> {
        match ty.deref().as_ref() {
            TypeKind::Forall(forall) => {
                // Determines if a hole should be lack or not checking if it has effect kind.
                let arg = if forall.kind.is_row() {
                    env.lacks(forall.name.clone(), Default::default())
                } else {
                    env.hole(forall.kind.clone(), forall.name.clone())
                };

                let kind = forall.kind.clone();

                // Applies the body using the hole argument.
                forall.body.apply(Some(forall.name.clone()), arg, kind)
            }
            _ => ty.clone(),
        }
    }

    /// Generalizes a monotype to a poly type.
    pub fn generalize(&mut self, env: &Env, ty: &Type<Virtual>) -> Type<Virtual> {
        fn go(level: Level, ty: Type<Real>, new_vars: &mut Vec<(Symbol, Type<Real>)>) {
            match ty.as_ref() {
                TypeKind::Arrow(p) => {
                    go(level, p.ty.clone(), new_vars);
                    go(level, p.effs.clone(), new_vars);
                    go(level.inc(), p.body.clone(), new_vars);
                }
                TypeKind::Forall(forall) => {
                    go(level, forall.kind.clone(), new_vars);
                    go(level.inc(), forall.body.clone(), new_vars);
                }
                TypeKind::Hole(hole) => match hole.0.borrow().clone() {
                    HoleInner::Empty(n, k, _) => {
                        new_vars.push((n, k));
                        let arg = Type::new(TypeKind::Bound(Index(new_vars.len() - 1 + level.0)));
                        hole.0.replace(HoleInner::Filled(arg));
                    }
                    HoleInner::Row(n, _, _) => {
                        new_vars.push((n, Type::new(TypeKind::Row)));
                        let arg = Type::new(TypeKind::Bound(Index(new_vars.len() - 1 + level.0)));
                        hole.0.replace(HoleInner::Filled(arg));
                    }

                    HoleInner::Filled(filled) => go(level, filled, new_vars),
                },
                TypeKind::Tuple(t) => {
                    for ty in t.iter() {
                        go(level, ty.clone(), new_vars);
                    }
                }
                TypeKind::Application(f, a) => {
                    go(level, f.clone(), new_vars);
                    go(level, a.clone(), new_vars);
                }
                TypeKind::Extend(_, t, u) => {
                    go(level, t.clone(), new_vars);
                    go(level, u.clone(), new_vars);
                }
                TypeKind::Type => (),
                TypeKind::Effect => (),
                TypeKind::Empty => (),
                TypeKind::Bound(_) => (),
                TypeKind::Variable(_) => (),
                TypeKind::Error => (),
                TypeKind::Row => (),
            }
        }

        let mut vars = Vec::new();

        let real = ty.clone().quote(env.level);

        go(env.level, real.clone(), &mut vars);

        let real = vars.iter().fold(real, |rest, (name, kind)| {
            Type::forall(real::Forall {
                name: name.clone(),
                kind: kind.clone(),
                body: rest,
            })
        });

        real.eval(env)
    }
}
