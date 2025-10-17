use std::{
    collections::{HashMap, hash_map::Entry},
    str::FromStr,
};

use itertools::Itertools;
use lime_generic_def::{
    CellPat, Function, Gate, InputIndices, InstructionType, NaryPat, OperandPat, Pats, Range,
    TuplePat, TuplePats, TuplesDef,
};
use proc_macro2::Span;
use quote::{ToTokens, quote};
use syn::{Error, Result};

use crate::generic::{Outputs, TuplesDefValue};

use super::{
    CellType,
    ast::{self},
    krate,
    operands::NamedOperands,
};

pub struct InstructionTypes(pub HashMap<String, InstructionType<CellType>>);

impl InstructionTypes {
    pub fn new(operands: &NamedOperands, ast: &ast::Architecture) -> Result<Self> {
        let mut result = HashMap::new();
        for (id, instruction) in ast.inner.instructions.value.iter().enumerate() {
            let name = instruction.name.to_string();
            let Entry::Vacant(entry) = result.entry(name) else {
                return Err(Error::new(
                    instruction.name.span(),
                    "duplicate instruction name",
                ));
            };
            let input = operand_tuples_to_cell_tuples(
                instruction.input.value.operands.span(),
                &operands.by_ident(&instruction.input.value.operands)?,
            )?;
            let input_override = instruction
                .input_target_idx
                .as_ref()
                .map(|range| InputIndices::try_from(range))
                .transpose()?
                .unwrap_or(InputIndices::None);
            let input_inverted = instruction
                .input
                .value
                .inverted_range
                .as_ref()
                .map(|range| InputIndices::try_from(range))
                .transpose()?
                .unwrap_or(InputIndices::None);
            if let Some(range) = &instruction.function.forwarded {
                return Err(Error::new(
                    range.span(),
                    "specifiying a range here is not (yet) supported",
                ));
            }
            let function = (&instruction.function).try_into()?;
            entry.insert(InstructionType {
                id: id as u8,
                name: instruction.name.to_string().into(),
                input,
                input_override,
                input_inverted,
                input_range: Range { start: 0 },
                function,
                outputs: Outputs::new(operands, &instruction.output)?.0,
            });
        }
        Ok(Self(result))
    }
}

impl ToTokens for InstructionTypes {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let krate = krate();
        let instructions = self.0.values().map(InstructionTypeValue);
        tokens.extend(quote! {
            #krate::InstructionTypes::new(vec![
                #(#instructions),*
            ])
        });
    }
}

struct InstructionTypeValue<'a>(&'a InstructionType<CellType>);

impl ToTokens for InstructionTypeValue<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let InstructionType {
            id,
            name,
            input,
            input_override,
            input_range,
            input_inverted,
            function,
            outputs,
        } = &self.0;
        let (input, input_override, input_inverted, function, range, outputs) = (
            TuplesDefValue(input),
            InputIndicesValue(input_override),
            InputIndicesValue(input_inverted),
            FunctionValue(*function),
            RangeValue(*input_range),
            Outputs(outputs.clone()),
        );
        let krate = krate();
        tokens.extend(quote! {
            #krate::InstructionType {
                id: #id,
                name: #name.into(),
                input: #input,
                input_override: #input_override,
                input_inverted: #input_inverted,
                input_range: #range,
                function: #function,
                outputs: #outputs,
            }
        });
    }
}

struct InputIndicesValue<'a>(&'a InputIndices);

impl ToTokens for InputIndicesValue<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let krate = krate();
        tokens.extend(match self.0 {
            InputIndices::None => quote!(#krate::InputIndices::None),
            InputIndices::All => quote!(#krate::InputIndices::All),
            InputIndices::Index(idx) => quote!(#krate::InputIndices::Index(#idx)),
        })
    }
}

impl TryFrom<&ast::Function> for Function {
    type Error = Error;

    fn try_from(value: &ast::Function) -> Result<Self> {
        Ok(Function {
            gate: Gate::try_from(&value.gate)?,
            inverted: value.inverted.is_some(),
        })
    }
}

struct FunctionValue(Function);

impl ToTokens for FunctionValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let krate = krate();
        let Function { inverted, gate } = self.0;
        let gate = GateValue(gate);
        tokens.extend(quote! {
            #krate::Function {
                gate: #gate,
                inverted: #inverted,
            }
        });
    }
}

struct RangeValue(Range);

impl ToTokens for RangeValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let start = self.0.start;
        let krate = krate();
        tokens.extend(quote!(#krate::Range { start: #start, }));
    }
}

impl TryFrom<&ast::BoolOrIdent> for Gate {
    type Error = Error;

    fn try_from(value: &ast::BoolOrIdent) -> Result<Self> {
        match value {
            ast::BoolOrIdent::Bool(lit) => Ok(Self::Constant(lit.value)),
            ast::BoolOrIdent::Ident(ident) => Gate::from_str(ident.to_string().as_str())
                .map_err(|_| Error::new(ident.span(), "unknown gate")),
        }
    }
}

struct GateValue(Gate);

impl ToTokens for GateValue {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let variant = match self.0 {
            Gate::And => quote!(And),
            Gate::Maj => quote!(Maj),
            Gate::Xor => quote!(Xor),
            Gate::Constant(c) => quote!(Constant(#c)),
        };
        let krate = krate();
        tokens.extend(quote!(#krate::Gate::#variant));
    }
}

fn operand_tuples_to_cell_tuples(
    error_span: Span,
    def: &TuplesDef<OperandPat<CellType>>,
) -> Result<TuplesDef<CellPat<CellType>>> {
    let from_pats = |pats: &Pats<OperandPat<CellType>>| -> Result<Pats<CellPat<CellType>>> {
        Ok(Pats::new(
            pats.iter()
                .map(|pat| {
                    if pat.inverted {
                        Err(Error::new(error_span, "operands contain inverted patterns"))
                    } else {
                        Ok(pat.cell.clone())
                    }
                })
                .try_collect()?,
        ))
    };
    let from_tuple_pat =
        |pat: &TuplePat<OperandPat<CellType>>| -> Result<TuplePat<CellPat<CellType>>> {
            Ok(TuplePat::new(pat.iter().map(from_pats).try_collect()?))
        };
    Ok(match def {
        TuplesDef::Nary(nary) => TuplesDef::Nary(NaryPat(from_pats(&nary.0)?)),
        TuplesDef::Tuples(tuples) => TuplesDef::Tuples(TuplePats::new(
            tuples.iter().map(from_tuple_pat).try_collect()?,
        )),
    })
}
