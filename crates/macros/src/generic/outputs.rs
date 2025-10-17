use itertools::Itertools;
use lime_generic_def as def;
use quote::{ToTokens, quote};
use syn::{Ident, Result};

use crate::generic::TuplesDefValue;

use super::{CellType, ast, krate, operands::NamedOperands};

pub struct Outputs(pub def::Outputs<CellType>);

impl Outputs {
    pub fn new(operands: &NamedOperands, names: &Option<ast::Tuple<Ident>>) -> Result<Self> {
        let outputs = names
            .iter()
            .flat_map(|outputs| outputs.value.iter())
            .map(|output| operands.by_ident(output))
            .try_collect()?;
        Ok(Self(def::Outputs::new(outputs)))
    }
}

impl ToTokens for Outputs {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let krate = krate();
        let outputs = self.0.iter().map(TuplesDefValue);
        tokens.extend(quote! {
            #krate::Outputs::new(vec![
                #(#outputs),*
            ])
        });
    }
}
