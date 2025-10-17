use derive_more::{Deref, DerefMut};
use itertools::Itertools;
use lime_generic_def::{CellPat, NaryPat, OperandPat, Pats, TuplePat, TuplePats, TuplesDef};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use std::collections::BTreeMap;
use syn::{Error, Ident, Result};

use super::{
    CellType, Cells, ast,
    ast::{NameAndOpTuplesDef, OperandTuplesElement},
    krate,
};

#[derive(Deref, DerefMut)]
pub struct NamedOperands(pub BTreeMap<String, TuplesDef<OperandPat<CellType>>>);

impl NamedOperands {
    pub fn new(cells: &Cells, ast: &ast::Architecture) -> Result<Self> {
        let mut result = BTreeMap::new();
        for NameAndOpTuplesDef { name, operands, .. } in ast.inner.operands.value.iter() {
            let operands = match operands {
                ast::OpTuplesDef::Tuples { tuples, .. } => {
                    let mut vec = Vec::new();
                    let mut arity = None;
                    for element in tuples {
                        match element {
                            OperandTuplesElement::Tuple(tuple) => {
                                if let Some(arity) = arity
                                    && tuple.value.len() != arity
                                {
                                    return Err(Error::new(
                                        tuple.paren.span.join(),
                                        "tuple does not match arity with previous tuples",
                                    ));
                                }
                                arity = Some(tuple.value.len());
                                vec.push(TuplePat::new(
                                    tuple
                                        .value
                                        .iter()
                                        .map(|typ| cells.new_operand_types(typ))
                                        .try_collect()?,
                                ))
                            }
                            OperandTuplesElement::Ref { name, .. } => {
                                if let Some(operands) = result.get(name.to_string().as_str()) {
                                    let TuplesDef::Tuples(tuples) = operands else {
                                        return Err(Error::new(
                                            name.span(),
                                            "can only expand tuple operands",
                                        ));
                                    };
                                    if arity.is_some_and(|arity| arity != tuples.arity()) {
                                        return Err(Error::new(
                                            name.span(),
                                            "arity of referenced operands does not match",
                                        ));
                                    }
                                    arity = Some(tuples.arity());
                                    vec.extend(tuples.iter().cloned());
                                } else {
                                    return Err(Error::new(name.span(), "unknown operands name"));
                                }
                            }
                        }
                    }
                    TuplesDef::Tuples(TuplePats::new(vec))
                }
                ast::OpTuplesDef::Nary { types, .. } => {
                    TuplesDef::Nary(NaryPat(cells.new_operand_types(types)?))
                }
            };
            if result.insert(name.to_string(), operands).is_some() {
                return Err(Error::new(name.span(), "duplicate operands name"));
            }
        }
        Ok(Self(result))
    }
    pub fn by_ident(&self, name: &Ident) -> Result<TuplesDef<OperandPat<CellType>>> {
        self.get(&name.to_string())
            .cloned()
            .ok_or_else(|| Error::new(name.span(), "unknown operands"))
    }
}

pub struct TuplesDefValue<'a, P>(pub &'a TuplesDef<P>);

impl<P: ToTokenPat> ToTokens for TuplesDefValue<'_, P> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let krate = krate();
        match &self.0 {
            TuplesDef::Nary(nary) => {
                let inner = NaryPatValue(nary);
                tokens.extend(quote!(#krate::TuplesDef::Nary(#inner)));
            }
            TuplesDef::Tuples(tuples) => {
                let inner = TuplePatsValue(tuples);
                tokens.extend(quote!(#krate::TuplesDef::Tuples(#inner)));
            }
        }
    }
}

struct NaryPatValue<'a, P>(&'a NaryPat<P>);

impl<P: ToTokenPat> ToTokens for NaryPatValue<'_, P> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let krate = krate();
        let inner = PatsValue(&self.0.0);
        tokens.extend(quote!(#krate::NaryPat(#inner)));
    }
}

struct TuplePatsValue<'a, P>(&'a TuplePats<P>);

impl<P: ToTokenPat> ToTokens for TuplePatsValue<'_, P> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let krate = krate();
        let pats = self.0.iter().map(TuplePatValue);
        tokens.extend(quote! {
            #krate::TuplePats::new(vec![
                #(#pats),*
            ])
        });
    }
}

struct TuplePatValue<'a, P>(&'a TuplePat<P>);

impl<P: ToTokenPat> ToTokens for TuplePatValue<'_, P> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let krate = krate();
        let pats = self.0.iter().map(PatsValue);
        tokens.extend(quote! {
            #krate::TuplePat::new(vec![
                #(#pats),*
            ])
        });
    }
}

struct PatsValue<'a, P>(&'a Pats<P>);

impl<'a, P: ToTokenPat> ToTokens for PatsValue<'a, P> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let krate = krate();
        let pats = self.0.iter().map(|pat| pat.to_value());
        tokens.extend(quote! {
            #krate::Pats::new(vec![#(#pats),*])
        });
    }
}

struct CellPatValue<'a>(&'a CellPat<CellType>);

impl ToTokens for CellPatValue<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let krate = krate();
        tokens.extend(match &self.0 {
            CellPat::Type(typ) => quote!(#krate::CellPat::Type(#typ)),
            CellPat::Cell(cell) => {
                let typ = cell.clone().typ();
                let idx = cell.clone().index();
                quote!(#krate::CellPat::Cell(#krate::Cell::new(#typ, #idx)))
            }
        })
    }
}

impl ToTokenPat for CellPat<CellType> {
    type Value<'a> = CellPatValue<'a>;

    fn to_value(&self) -> Self::Value<'_> {
        CellPatValue(self)
    }
}

struct OperandPatValue<'a>(&'a OperandPat<CellType>);

impl ToTokens for OperandPatValue<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let krate = krate();
        let OperandPat { inverted, cell } = &self.0;
        let cell = CellPatValue(cell);
        tokens.extend(quote! {
            #krate::OperandPat {
                cell: #cell,
                inverted: #inverted,
            }
        });
    }
}

trait ToTokenPat {
    type Value<'a>: ToTokens
    where
        Self: 'a;

    fn to_value(&self) -> Self::Value<'_>;
}

impl ToTokenPat for OperandPat<CellType> {
    type Value<'a> = OperandPatValue<'a>;

    fn to_value(&self) -> Self::Value<'_> {
        OperandPatValue(self)
    }
}
