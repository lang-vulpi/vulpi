//! Module for unification and subsumption of types.

#![allow(clippy::only_used_in_recursion)]

use crate::{context::Context, errors::TypeErrorKind};

use super::{
    eval::Quote,
    r#virtual::Pi,
    r#virtual::{Env, Virtual},
    Hole, HoleInner, Level, Type, TypeKind,
};

type Result<T = ()> = std::result::Result<T, TypeErrorKind>;

impl Context {
    pub fn subsumes(&mut self, env: Env, left: Type<Virtual>, right: Type<Virtual>) {
        fn go(ctx: &mut Context, env: Env, left: Type<Virtual>, right: Type<Virtual>) -> Result {
            let l = left.deref();
            let r = right.deref();

            match (l.as_ref(), r.as_ref()) {
                (TypeKind::Hole(n), _) if n.is_empty() => {
                    ctx.sub_hole_type(env, n.clone(), r.clone())
                }
                (_, TypeKind::Hole(n)) if n.is_empty() => {
                    ctx.sub_type_hole(env, l.clone(), n.clone())
                }
                (TypeKind::Arrow(m), TypeKind::Arrow(n)) => {
                    // Change due to variance.
                    go(ctx, env.clone(), n.typ.clone(), m.typ.clone())?;
                    go(ctx, env, m.body.clone(), n.body.clone())
                }
                (_, TypeKind::Forall(forall)) => {
                    let lvl_ty = Type::new(TypeKind::Bound(env.level));
                    go(
                        ctx,
                        env.add(None, lvl_ty.clone()),
                        l.clone(),
                        forall.body.apply_local(None, lvl_ty),
                    )
                }
                (TypeKind::Forall(_), _) => {
                    let instantiated = ctx.instantiate(&env, &l);
                    go(ctx, env, instantiated, r.clone())
                }
                (_, _) => ctx.unify(env, l, r),
            }
        }

        let result = go(self, env.clone(), left.clone(), right.clone());

        if let Err(kind) = result {
            match kind {
                TypeErrorKind::TypeMismatch(_, _, _) => self.report(
                    &env,
                    TypeErrorKind::TypeMismatch(
                        env.clone(),
                        left.quote(env.level),
                        right.quote(env.level),
                    ),
                ),
                _ => self.report(&env, kind),
            }
        }
    }

    fn sub_hole_type(&mut self, env: Env, left: Hole<Virtual>, right: Type<Virtual>) -> Result {
        match right.deref().as_ref() {
            TypeKind::Forall(forall) => {
                let lvl_ty = Type::new(TypeKind::Bound(env.level));
                self.sub_hole_type(
                    env.add(Some(forall.name.clone()), lvl_ty.clone()),
                    left,
                    forall.body.apply_local(None, lvl_ty),
                )
            }
            TypeKind::Arrow(pi) => {
                let HoleInner::Empty(_, kind, _) = left.0.borrow().clone() else {
                    unreachable!()
                };

                let hole_a = self.hole(&env, kind.clone());
                let hole_b = self.hole(&env, kind);

                left.fill(Type::new(TypeKind::Arrow(Pi {
                    typ: hole_a.clone(),
                    body: hole_b.clone(),
                })));

                let a = pi.typ.clone();
                let b = pi.body.clone();

                let TypeKind::Hole(hole_a) = hole_a.as_ref() else {
                    unreachable!()
                };
                let TypeKind::Hole(hole_b) = hole_b.as_ref() else {
                    unreachable!()
                };

                self.sub_type_hole(env.clone(), a, hole_a.clone())?;
                self.sub_hole_type(env, hole_b.clone(), b)
            }
            _ => self.unify_hole(env, left, right),
        }
    }

    fn sub_type_hole(&mut self, env: Env, left: Type<Virtual>, right: Hole<Virtual>) -> Result {
        let deref = &left.deref();
        match deref.as_ref() {
            TypeKind::Forall(_) => {
                let left = self.instantiate(&env, deref);
                self.sub_type_hole(env, left, right)
            }
            TypeKind::Arrow(pi) => {
                let HoleInner::Empty(_, kind, _) = right.0.borrow().clone() else {
                    unreachable!()
                };

                let hole_a = self.hole(&env, kind.clone());
                let hole_b = self.hole(&env, kind);

                right.fill(Type::new(TypeKind::Arrow(Pi {
                    typ: hole_a.clone(),
                    body: hole_b.clone(),
                })));

                let a = pi.typ.clone();
                let b = pi.body.clone();

                let TypeKind::Hole(hole_a) = hole_a.as_ref() else {
                    unreachable!()
                };
                let TypeKind::Hole(hole_b) = hole_b.as_ref() else {
                    unreachable!()
                };

                self.sub_hole_type(env.clone(), hole_a.clone(), a)?;
                self.sub_type_hole(env, b, hole_b.clone())
            }
            _ => self.unify_hole(env, right, left),
        }
    }

    pub fn overlaps(&mut self, env: Env, left: Type<Virtual>, right: Type<Virtual>) -> bool {
        let result = self.unify(env, left, right);
        result.is_ok()
    }

    pub fn unify(&mut self, env: Env, left: Type<Virtual>, right: Type<Virtual>) -> Result {
        let l = left.deref();
        let r = right.deref();
        match (l.as_ref(), r.as_ref()) {
            (TypeKind::Tuple(x), TypeKind::Tuple(y)) if x.len() == y.len() => x
                .iter()
                .zip(y.iter())
                .try_for_each(|(x, y)| self.unify(env.clone(), x.clone(), y.clone())),
            (TypeKind::Application(f, a), TypeKind::Application(g, b)) => {
                self.unify(env.clone(), f.clone(), g.clone())?;
                self.unify(env, a.clone(), b.clone())
            }
            (TypeKind::Qualified(f, u), TypeKind::Qualified(f1, u1)) => {
                self.unify(env.clone(), f.clone(), f1.clone())?;
                self.unify(env, u.clone(), u1.clone())
            }
            (TypeKind::Hole(n), TypeKind::Hole(m)) if n == m => Ok(()),
            (TypeKind::Hole(m), _) => self.unify_hole(env, m.clone(), r),
            (_, TypeKind::Hole(m)) => self.unify_hole(env, m.clone(), l),
            (TypeKind::Bound(x), TypeKind::Bound(y)) if x == y => Ok(()),
            (TypeKind::Variable(x), TypeKind::Variable(y)) if x == y => Ok(()),
            (TypeKind::Type, TypeKind::Type) => Ok(()),
            (TypeKind::Constraint, TypeKind::Constraint) => Ok(()),
            (TypeKind::Error, _) | (_, TypeKind::Error) => Ok(()),
            (_, _) => Err(TypeErrorKind::TypeMismatch(
                env.clone(),
                left.quote(env.level),
                right.quote(env.level),
            )),
        }
    }

    fn occurs(&self, env: Env, scope: &Level, hole: Hole<Virtual>, typ: Type<Virtual>) -> Result {
        match typ.deref().as_ref() {
            TypeKind::Arrow(pi) => {
                self.occurs(env.clone(), scope, hole.clone(), pi.typ.clone())?;
                self.occurs(env, scope, hole, pi.body.clone())
            }
            TypeKind::Forall(forall) => {
                let lvl_ty = Type::new(TypeKind::Bound(env.level));
                self.occurs(env, scope, hole, forall.body.apply_local(None, lvl_ty))
            }
            TypeKind::Hole(h) if h.clone() == hole => Err(TypeErrorKind::InfiniteType),
            TypeKind::Bound(l) if l >= scope => Err(TypeErrorKind::EscapingScope),
            TypeKind::Tuple(t) => t
                .iter()
                .try_for_each(|t| self.occurs(env.clone(), scope, hole.clone(), t.clone())),
            TypeKind::Application(f, a) => {
                self.occurs(env.clone(), scope, hole.clone(), f.clone())?;
                self.occurs(env, scope, hole, a.clone())
            }
            _ => Ok(()),
        }
    }

    fn unify_hole(&mut self, env: Env, hole: Hole<Virtual>, right: Type<Virtual>) -> Result {
        let borrow = hole.0.borrow().clone();
        match borrow {
            HoleInner::Empty(_, _, lvl) => match right.deref().as_ref() {
                TypeKind::Hole(hole1) if hole == hole1.clone() => Ok(()),
                _ => {
                    self.occurs(env, &lvl, hole.clone(), right.clone())?;
                    hole.fill(right);
                    Ok(())
                }
            },
            HoleInner::Filled(f) => self.unify(env, f, right),
        }
    }
}
