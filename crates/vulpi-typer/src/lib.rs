//! This is the entrypoint for the `vulpi-typer` crate. It is responsible for type checking a
//! higher rank, higher kinded, algebraic type system. It is also responsible for type inference
//! and type checking of the ambient effects system.

use std::collections::HashMap;

pub use context::Context;
use r#type::real::Arrow;
use r#type::TypeKind;
pub use r#type::{r#virtual::Env, Type};
pub use r#type::{r#virtual::Virtual, real::Real, Kind};
use vulpi_intern::Symbol;
use vulpi_syntax::elaborated::Decl;
use vulpi_syntax::r#abstract::LetDecl;

use crate::r#type::eval::{Eval, Quote};
use crate::r#type::real::Forall;
use crate::r#type::Index;
use infer::Infer;
use module::{Def, LetDef, TypeData};

use vulpi_syntax::{
    elaborated,
    r#abstract::{EffectDecl, ExternalDecl, Module, ModuleDecl, TypeDecl, TypeDef},
};

pub mod context;
pub mod errors;
pub mod infer;
pub mod module;
pub mod r#type;

/// Trait for declaration of top level items inside the type checker.
pub trait Declare {
    fn declare(&self, context: (&mut Context, Env));
    fn define(&self, _context: (&mut Context, Env)) {}
}

impl Declare for TypeDecl {
    fn declare(&self, (context, env): (&mut Context, Env)) {
        let vec = &self.binders;

        let mut names = Vec::new();
        let mut binders = Vec::new();

        for binder in vec {
            let (n, binder) = binder.infer((context, env.clone()));
            binders.push(binder.eval(&env));
            names.push(n);
        }

        let kind = Type::<Virtual>::function(binders.clone(), Type::typ());

        let type_def = &self.def;
        let def = get_definition_of_type(type_def);

        context.modules.get(&self.name.path).types.insert(
            self.name.name.clone(),
            TypeData {
                kind,
                binders: names.into_iter().zip(binders.into_iter()).collect(),
                module: self.namespace.clone(),
                def,
            },
        );
    }

    fn define(&self, (ctx, mut env): (&mut Context, Env)) {
        let type_decl = ctx.modules.typ(&self.name);

        for (name, binder) in &type_decl.binders {
            env = env.add(Some(name.clone()), binder.clone());
        }

        let ret_type = Type::<Real>::application(
            Type::variable(self.name.clone()),
            (0..type_decl.binders.len())
                .rev()
                .map(|x| Type::bound(Index(x)))
                .collect(),
        );

        let decl = match &self.def {
            TypeDef::Sum(cons) => {
                let mut constructors = Vec::new();

                let mut cons_types = Vec::new();

                for cons in &cons.constructors {
                    constructors.push(cons.name.clone());

                    let mut types = Vec::new();

                    for arg in &cons.args {
                        env.on(arg.span.clone());
                        let (typ, kind) = arg.infer((ctx, env.clone()));
                        ctx.subsumes(env.clone(), kind, Kind::typ());
                        types.push(typ);
                    }

                    let typ = Type::<Real>::function(types, ret_type.clone());
                    cons_types.push((cons.name.clone(), cons.args.len(), typ));
                }

                for (name, arity, mut cons_typ) in cons_types {
                    for (name, binder) in type_decl.binders.iter().rev() {
                        cons_typ = Type::forall(Forall {
                            name: name.clone(),
                            kind: binder.clone().quote(env.level),
                            body: cons_typ,
                        });
                    }

                    ctx.modules
                        .get(&name.path)
                        .constructors
                        .insert(name.name.clone(), (cons_typ.eval(&env), arity));
                }

                elaborated::TypeDecl::Enum(constructors)
            }
            TypeDef::Record(rec) => {
                let mut types = Vec::new();
                let mut names = Vec::new();

                for field in &rec.fields {
                    names.push(field.0.clone());

                    let (typ, kind) = field.1.infer((ctx, env.clone()));
                    env.on(field.1.span.clone());

                    ctx.subsumes(env.clone(), kind, Kind::typ());

                    types.push(typ);
                }

                for (name, mut typ) in names.iter().zip(types.into_iter()) {
                    for (name, binder) in type_decl.binders.iter().rev() {
                        typ = Type::forall(Forall {
                            name: name.clone(),
                            kind: binder.clone().quote(env.level),
                            body: typ,
                        });
                    }

                    ctx.modules
                        .get(&name.path)
                        .fields
                        .insert(name.name.clone(), typ.eval(&env));
                }

                elaborated::TypeDecl::Record(names)
            }
            TypeDef::Synonym(_) => todo!(),
            TypeDef::Abstract => elaborated::TypeDecl::Abstract,
        };

        ctx.elaborated
            .decls
            .insert(self.name.clone(), elaborated::Decl::Type(decl));
    }
}

fn get_definition_of_type(type_def: &TypeDef) -> Def {
    match type_def {
        TypeDef::Sum(cons) => Def::Enum(cons.constructors.iter().map(|x| x.name.clone()).collect()),
        TypeDef::Record(rec) => Def::Record(rec.fields.iter().map(|x| x.0.clone()).collect()),
        TypeDef::Synonym(_) => Def::Type,
        TypeDef::Abstract => Def::Type,
    }
}

impl Declare for ExternalDecl {
    fn declare(&self, (ctx, mut env): (&mut Context, Env)) {
        let fvs = self.ty.data.free_variables();

        let mut unbound = Vec::new();

        for fv in fvs {
            let ty = ctx.hole(&env, Type::typ());
            env = env.add(Some(fv.clone()), ty.clone());
            unbound.push((fv, ty))
        }

        let (typ, k) = self.ty.infer((ctx, env.clone()));
        ctx.subsumes(env.clone(), k, Kind::typ());

        ctx.modules.get(&self.namespace).variables.insert(
            self.name.clone(),
            LetDef {
                typ: typ.eval(&env),
                binders: Default::default(),
                unbound,
                unbound_effects: vec![],
            },
        );
    }
}

impl Declare for EffectDecl {
    fn declare(&self, (context, env): (&mut Context, Env)) {
        let mut binders = Vec::new();
        let mut names = Vec::new();

        for binder in &self.binders {
            let (n, binder) = binder.infer((context, env.clone()));
            binders.push(binder.eval(&env));
            names.push(n);
        }

        let kind = Type::<Virtual>::function(binders.clone(), Type::effect());

        let effects = self
            .effects
            .iter()
            .map(|x| x.name.clone())
            .collect::<Vec<_>>();

        context.modules.get(&self.name.path.clone()).types.insert(
            self.name.name.clone(),
            TypeData {
                kind,
                binders: names.into_iter().zip(binders.into_iter()).collect(),
                module: self.namespace.clone(),
                def: Def::Effect(effects.clone()),
            },
        );

        context
            .elaborated
            .decls
            .insert(self.name.clone(), Decl::Effect(effects));
    }

    fn define(&self, (ctx, mut env): (&mut Context, Env)) {
        let type_decl = ctx.modules.typ(&self.name);

        for (name, binder) in &type_decl.binders {
            env = env.add(Some(name.clone()), binder.clone());
        }

        let mut names = Vec::new();
        let mut eff_types = Vec::new();

        for eff in &self.effects {
            names.push(eff.name.clone());

            let mut fvs = eff.ty.data.free_variables();
            let mut bound = Vec::new();

            for arg in &eff.args {
                fvs.extend(arg.data.free_variables());
            }

            for (name, _) in &type_decl.binders {
                fvs.remove(name);
            }

            let size = fvs.len();

            for fv in fvs {
                let ty = ctx.hole(&env, Type::typ());
                env = env.add(Some(fv.clone()), ty.clone());
                bound.push((fv, ty));
            }

            let extension_type_name = ctx.new_name();
            let hole = ctx.hole(&env, Type::typ());
            env = env.add(Some(extension_type_name.clone()), hole);

            let eff_type = Type::<Real>::application(
                Type::variable(self.name.clone()),
                (0..type_decl.binders.len())
                    .rev()
                    .map(|x| Type::bound(Index(x + size + 1)))
                    .collect(),
            );

            let mut types = Vec::new();

            for arg in &eff.args {
                env.on(arg.span.clone());
                let (typ, kind) = arg.infer((ctx, env.clone()));
                ctx.subsumes(env.clone(), kind, Kind::typ());
                types.push(typ);
            }

            if eff.args.is_empty() {
                ctx.report(&env, errors::TypeErrorKind::AtLeastOneArgument)
            } else {
                let (ret_type, kind) = eff.ty.infer((ctx, env.clone()));
                env.on(eff.ty.span.clone());
                ctx.subsumes(env.clone(), kind, Kind::typ());

                let popped = types.pop().unwrap();

                let extension_type = Type::<Real>::bound(Index(0));

                let fun = Type::new(TypeKind::<Real>::Arrow(Arrow {
                    ty: popped,
                    effs: Type::<Real>::extend(
                        self.name.clone(),
                        eff_type.clone(),
                        extension_type.clone(),
                    ),
                    body: ret_type,
                }));

                let mut typ = Type::<Real>::function(types, fun.clone());

                typ = Type::forall(Forall {
                    name: extension_type_name.clone(),
                    kind: Type::row(),
                    body: typ,
                });

                for (name, kind) in bound {
                    typ = Type::forall(Forall {
                        name,
                        kind: kind.quote(env.level),
                        body: typ,
                    });
                }

                eff_types.push((eff.args.len(), typ));
            }
        }

        for (name, (arity, mut cons_typ)) in names.iter().zip(eff_types.into_iter()) {
            for (name, binder) in type_decl.binders.iter().rev() {
                cons_typ = Type::forall(Forall {
                    name: name.clone(),
                    kind: binder.clone().quote(env.level),
                    body: cons_typ,
                });
            }

            ctx.modules
                .get(&name.path)
                .effects
                .insert(name.name.clone(), (cons_typ.eval(&env), arity));
        }
    }
}

impl Declare for LetDecl {
    fn declare(&self, (ctx, mut env): (&mut Context, Env)) {
        let has_effect = self.ret.as_ref().map(|x| x.0.is_some()).unwrap_or_default();

        if has_effect && self.binders.is_empty() {
            ctx.report(&env, errors::TypeErrorKind::AtLeastOneArgument);
            return;
        }

        let mut efvs = self
            .ret
            .as_ref()
            .and_then(|x| {
                x.0.as_ref()
                    .and_then(|x| x.rest.as_ref().map(|x| x.data.free_effects()))
            })
            .unwrap_or_default();

        let mut fvs = self
            .ret
            .as_ref()
            .map(|x| x.1.data.free_variables())
            .unwrap_or_default();

        for arg in &self.binders {
            fvs.extend(arg.ty.data.free_variables());
            efvs.extend(arg.ty.data.free_effects());
        }

        let mut bound = Vec::new();
        let mut effect_bounds = Vec::new();

        for fv in fvs {
            let ty = ctx.hole(&env, Type::typ());
            env = env.add(Some(fv.clone()), ty.clone());
            bound.push((fv, ty));
        }

        for fv in efvs {
            let ty = ctx.lacks(&env, Default::default());
            env = env.add(Some(fv.clone()), ty.clone());
            effect_bounds.push((fv, ty));
        }

        let mut args = Vec::new();

        let mut binders: HashMap<Symbol, Type<Virtual>> = Default::default();

        for arg in &self.binders {
            let (ty, kind) = arg.ty.infer((ctx, env.clone()));
            env.on(arg.ty.span.clone());
            ctx.subsumes(env.clone(), kind, Kind::typ());

            let mut hash_map = Default::default();
            let pat_ty = arg.pattern.infer((ctx, &mut hash_map, env.clone()));

            binders.extend(hash_map);

            ctx.subsumes(env.clone(), pat_ty, ty.eval(&env));

            args.push(ty);
        }

        let (effs, ret) = if let Some((eff, ret)) = &self.ret {
            let effs = eff.infer((ctx, env.clone()));

            let (ty, kind) = ret.infer((ctx, env.clone()));
            env.on(ret.span.clone());
            ctx.subsumes(env.clone(), kind, Kind::typ());

            (effs, ty)
        } else {
            (Type::new(TypeKind::Empty), ctx.hole(&env, Kind::typ()))
        };

        let ret = if has_effect {
            let ty = args.pop().unwrap();

            Type::new(TypeKind::Arrow(Arrow {
                ty,
                effs,
                body: ret,
            }))
        } else {
            ret
        };

        let mut typ = Type::<Real>::function(args.clone(), ret);

        for (name, _) in effect_bounds.clone() {
            typ = Type::forall(Forall {
                name,
                kind: Type::row(),
                body: typ,
            });
        }

        for (name, kind) in bound.clone() {
            typ = Type::forall(Forall {
                name,
                kind: kind.quote(env.level),
                body: typ,
            });
        }

        ctx.modules.get(&self.name.path.clone()).variables.insert(
            self.name.name.clone(),
            LetDef {
                typ: typ.eval(&env),
                binders,
                unbound: bound,
                unbound_effects: effect_bounds,
            },
        );
    }
}

impl Declare for ModuleDecl {
    fn declare(&self, (ctx, env): (&mut Context, Env)) {
        if let Some(modules) = self.modules() {
            for decl in modules {
                decl.declare((ctx, env.clone()));
            }
        }

        if let Some(types) = self.types() {
            for decl in types {
                decl.declare((ctx, env.clone()));
            }
        }

        if let Some(effects) = self.effects() {
            for decl in effects {
                decl.declare((ctx, env.clone()));
            }
        }

        if let Some(externals) = self.externals() {
            for decl in externals {
                decl.declare((ctx, env.clone()));
            }
        }

        if let Some(lets) = self.lets() {
            for decl in lets {
                decl.declare((ctx, env.clone()));
            }
        }
    }

    fn define(&self, (ctx, env): (&mut Context, Env)) {
        if let Some(modules) = self.modules() {
            for decl in modules {
                decl.declare((ctx, env.clone()));
            }
        }

        if let Some(types) = self.types() {
            for decl in types {
                decl.define((ctx, env.clone()));
            }
        }

        if let Some(effects) = self.effects() {
            for decl in effects {
                decl.define((ctx, env.clone()));
            }
        }

        if let Some(externals) = self.externals() {
            for decl in externals {
                decl.define((ctx, env.clone()));
            }
        }

        if let Some(lets) = self.lets() {
            for decl in lets {
                decl.declare((ctx, env.clone()));
            }
        }
    }
}

impl Declare for Module {
    fn declare(&self, (ctx, env): (&mut Context, Env)) {
        for module in self.modules() {
            module.declare((ctx, env.clone()));
        }

        for decl in self.types() {
            decl.declare((ctx, env.clone()));
        }

        for decl in self.effects() {
            decl.declare((ctx, env.clone()));
        }

        for decl in self.lets() {
            decl.declare((ctx, env.clone()));
        }

        for externals in self.externals() {
            externals.declare((ctx, env.clone()));
        }
    }

    fn define(&self, (ctx, env): (&mut Context, Env)) {
        for module in self.modules() {
            module.define((ctx, env.clone()));
        }

        for decl in self.effects() {
            decl.define((ctx, env.clone()));
        }

        for decl in self.types() {
            decl.define((ctx, env.clone()));
        }

        for decl in self.lets() {
            decl.define((ctx, env.clone()));
        }
    }
}
