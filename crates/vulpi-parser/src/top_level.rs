use vulpi_syntax::{concrete::top_level::*, tokens::TokenData};

use crate::{Parser, Result};

impl<'a> Parser<'a> {
    pub fn binder(&mut self) -> Result<Binder> {
        let left_paren = self.expect(TokenData::LPar)?;
        let pattern = self.pattern()?;
        let colon = self.expect(TokenData::Colon)?;
        let typ = self.typ()?;
        let right_paren = self.expect(TokenData::RPar)?;
        Ok(Binder {
            left_paren,
            pattern,
            colon,
            typ,
            right_paren,
        })
    }

    pub fn trait_binder(&mut self) -> Result<TraitBinder> {
        let left_bracket = self.expect(TokenData::LBracket)?;
        let typ = self.typ()?;
        let right_bracket = self.expect(TokenData::RBracket)?;
        Ok(TraitBinder {
            left_bracket,
            typ,
            right_bracket,
        })
    }

    pub fn explicit_type_binder(&mut self) -> Result<ExplicitTypeBinder> {
        let name = self.lower()?;
        let colon = self.expect(TokenData::Colon)?;
        let kind = self.kind()?;
        Ok(ExplicitTypeBinder { name, colon, kind })
    }

    pub fn type_binder(&mut self) -> Result<TypeBinder> {
        if self.at(TokenData::LowerIdent) {
            let lower = self.lower()?;
            Ok(TypeBinder::Implicit(lower))
        } else {
            Ok(TypeBinder::Explicit(
                self.parenthesis(Self::explicit_type_binder)?,
            ))
        }
    }

    pub fn let_binder(&mut self) -> Result<LetBinder> {
        if self.at(TokenData::LBracket) {
            let binder = self.trait_binder()?;
            Ok(LetBinder::Trait(binder))
        } else {
            let binder = self.binder()?;
            Ok(LetBinder::Param(binder))
        }
    }

    pub fn let_case(&mut self) -> Result<LetCase> {
        let pipe = self.expect(TokenData::Bar)?;
        let arm = self.pattern_arm()?;
        Ok(LetCase { pipe, arm })
    }

    pub fn let_decl(&mut self, visibility: Visibility) -> Result<LetDecl> {
        let signature = self.let_signature(visibility)?;

        let body = if self.at(TokenData::Equal) {
            let eq = self.expect(TokenData::Equal)?;
            let expr = self.expr()?;
            LetMode::Body(eq, expr)
        } else if self.at(TokenData::Bar) {
            LetMode::Cases(self.many(Self::let_case)?)
        } else {
            self.unexpected()?
        };

        Ok(LetDecl { signature, body })
    }

    fn trait_decl(&mut self, visibility: Visibility) -> Result<TraitDecl> {
        let trait_ = self.expect(TokenData::Trait)?;
        let supers = self.many(Self::trait_binder)?;
        let name = self.upper()?;
        let binders = self.many(Self::type_binder)?;
        let where_ = self.expect(TokenData::Where)?;
        let body = self.block(|ctx| ctx.let_signature(Visibility::Private))?;
        Ok(TraitDecl {
            visibility,
            trait_,
            supers,
            name,
            binders,
            where_,
            body,
        })
    }

    fn trait_impl(&mut self) -> Result<TraitImpl> {
        let impl_ = self.expect(TokenData::Impl)?;
        let supers = self.many(Self::trait_binder)?;
        let name = self.path_upper()?;
        let types = self.many(Self::type_atom)?;
        let where_ = self.expect(TokenData::Where)?;
        let body = self.block(|ctx| ctx.let_decl(Visibility::Private))?;
        Ok(TraitImpl {
            impl_,
            supers,
            name,
            types,
            where_,
            body,
        })
    }

    fn let_signature(&mut self, visibility: Visibility) -> Result<LetSignature> {
        let let_ = self.expect(TokenData::Let)?;
        let name = self.lower()?;
        let binders = self.many(Self::let_binder)?;
        let ret = if self.at(TokenData::Colon) {
            let colon = self.bump();
            let typ = self.typ()?;
            Some((colon, typ))
        } else {
            None
        };

        Ok(LetSignature {
            visibility,
            let_,
            name,
            binders,
            ret,
        })
    }

    pub fn constructor_decl(&mut self) -> Result<Constructor> {
        let pipe = self.expect(TokenData::Bar)?;
        let name = self.upper()?;
        let args = self.many(Self::type_atom)?;

        let typ = if self.at(TokenData::Colon) {
            let colon = self.bump();
            let typ = self.typ()?;
            Some((colon, typ))
        } else {
            None
        };

        Ok(Constructor {
            pipe,
            name,
            args,
            typ,
        })
    }

    pub fn sum_decl(&mut self) -> Result<SumDecl> {
        let constructors = self.many(Self::constructor_decl)?;
        Ok(SumDecl { constructors })
    }

    pub fn field(&mut self) -> Result<Field> {
        let visibility = self.visibility()?;
        let name = self.lower()?;
        let colon = self.expect(TokenData::Colon)?;
        let typ = self.typ()?;
        Ok(Field {
            name,
            colon,
            typ,
            visibility,
        })
    }

    pub fn command_decl(&mut self) -> Result<CommandDecl> {
        let command = self.expect(TokenData::Command)?;
        let name = self.expect(TokenData::String)?;
        Ok(CommandDecl {
            command: command.symbol(),
            name: name.symbol(),
        })
    }

    pub fn record_decl(&mut self) -> Result<RecordDecl> {
        let left_brace = self.expect(TokenData::LBrace)?;
        let fields = self.sep_by(TokenData::Comma, Self::field)?;
        let right_brace = self.expect(TokenData::RBrace)?;

        Ok(RecordDecl {
            left_brace,
            fields,
            right_brace,
        })
    }

    pub fn type_def(&mut self) -> Result<TypeDef> {
        match self.token() {
            TokenData::Bar => self.sum_decl().map(TypeDef::Sum),
            TokenData::LBrace => self.record_decl().map(TypeDef::Record),
            _ => self.type_atom().map(TypeDef::Synonym),
        }
    }

    pub fn type_decl(&mut self, visibility: Visibility) -> Result<TypeDecl> {
        let type_ = self.expect(TokenData::Type)?;
        let name = self.upper()?;
        let binders = self.many(Self::type_binder)?;

        let def = if self.at(TokenData::Equal) {
            let eq = self.expect(TokenData::Equal)?;
            let def = self.type_def()?;
            Some((eq, def))
        } else {
            None
        };

        Ok(TypeDecl {
            type_,
            name,
            binders,
            def,
            visibility,
        })
    }

    pub fn use_alias(&mut self) -> Result<UseAlias> {
        let as_ = self.expect(TokenData::As)?;
        let alias = self.upper()?;
        Ok(UseAlias { as_, alias })
    }

    pub fn visibility(&mut self) -> Result<Visibility> {
        if self.at(TokenData::Pub) {
            Ok(Visibility::Public(self.bump()))
        } else {
            Ok(Visibility::Private)
        }
    }

    pub fn use_decl(&mut self, visibility: Visibility) -> Result<UseDecl> {
        let use_ = self.expect(TokenData::Use)?;
        let path = self.path_upper()?;

        let alias = if self.at(TokenData::As) {
            Some(self.use_alias()?)
        } else {
            None
        };

        Ok(UseDecl {
            use_,
            path,
            alias,
            visibility,
        })
    }

    pub fn mod_decl(&mut self, visibility: Visibility) -> Result<ModuleDecl> {
        let mod_ = self.expect(TokenData::Mod)?;
        let name = self.upper()?;

        let part = if self.at(TokenData::Where) {
            let where_ = self.expect(TokenData::Where)?;
            let top_levels = self.block(Self::top_level)?;

            Some(ModuleInline {
                name: name.clone(),
                where_,
                top_levels,
            })
        } else {
            None
        };

        Ok(ModuleDecl {
            visibility,
            mod_,
            name,
            part,
        })
    }

    pub fn external_decl(&mut self, visibility: Visibility) -> Result<ExtDecl> {
        let external = self.expect(TokenData::External)?;
        let name = self.lower()?;
        let colon = self.expect(TokenData::Colon)?;
        let typ = self.typ()?;
        let equal = self.expect(TokenData::Equal)?;
        let str = self.expect(TokenData::String)?;

        Ok(ExtDecl {
            visibility,
            external,
            name,
            colon,
            typ,
            equal,
            str,
        })
    }

    pub fn top_level(&mut self) -> Result<TopLevel> {
        let vis = self.visibility()?;
        match self.token() {
            TokenData::Let => self.let_decl(vis).map(Box::new).map(TopLevel::Let),
            TokenData::Type => self.type_decl(vis).map(Box::new).map(TopLevel::Type),
            TokenData::Use => self.use_decl(vis).map(Box::new).map(TopLevel::Use),
            TokenData::Impl => self.trait_impl().map(Box::new).map(TopLevel::Impl),
            TokenData::Trait => self.trait_decl(vis).map(Box::new).map(TopLevel::Trait),
            TokenData::Mod => self.mod_decl(vis).map(Box::new).map(TopLevel::Module),
            TokenData::Command => self.command_decl().map(Box::new).map(TopLevel::Command),
            TokenData::External => self
                .external_decl(vis)
                .map(Box::new)
                .map(TopLevel::External),
            _ => self.unexpected(),
        }
    }

    pub fn program(&mut self) -> Program {
        let mut top_levels = vec![];

        while !self.at(TokenData::Eof) {
            match self.top_level() {
                Ok(top_level) => top_levels.push(top_level),
                Err(err) => {
                    self.report(err);
                    let errs = self.recover(&[TokenData::Let, TokenData::Type, TokenData::Use]);
                    top_levels.push(TopLevel::Error(errs))
                }
            }
        }

        let eof = self.eat(TokenData::Eof);
        Program { top_levels, eof }
    }
}
