mod ast;
mod cells;
mod instructions;
mod operands;
mod outputs;

use std::rc::Rc;

use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Path, Result, parse_quote};

pub use self::{cells::*, instructions::*, operands::*, outputs::*};

pub struct Architecture {
    pub ast: Rc<ast::Architecture>,
    pub cells: Cells,
    #[expect(dead_code)]
    pub operands: NamedOperands,
    pub instructions: InstructionTypes,
}

impl TryFrom<ast::Architecture> for Architecture {
    type Error = syn::Error;

    fn try_from(ast: ast::Architecture) -> Result<Self> {
        let ast = Rc::new(ast);
        let cells = Cells::from_ast(ast.clone())?;
        let operands = NamedOperands::new(&cells, &ast)?;
        let instructions = InstructionTypes::new(&operands, &ast)?;
        Ok(Self {
            ast,
            cells,
            operands,
            instructions,
        })
    }
}

impl ToTokens for Architecture {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let vis = &self.ast.vis;
        let name = &self.ast.name;
        let krate = krate();
        let ct = cell_type_enum_name(name);
        let cells = &self.cells;
        let instructions = &self.instructions;

        let mut instr_ids = TokenStream::new();
        for (name, instr) in &instructions.0 {
            let ident = format_ident!("{name}_INSTRUCTION_ID");
            let id = instr.id;
            instr_ids.extend(quote! { pub const #ident: u8 = #id; });
        }

        tokens.extend(quote! {
            #cells

            #vis struct #name;

            impl #name {
                #instr_ids
                #vis fn instructions() -> #krate::InstructionTypes<#ct> {
                    #instructions
                }
                #vis fn new() -> #krate::Architecture<#ct> {
                    #krate::Architecture::new(Self::instructions())
                }
            }
        });
    }
}

pub fn define_generic_architecture(item: TokenStream) -> Result<TokenStream> {
    let ast: ast::Architecture = syn::parse2(item)?;
    let arch = Architecture::try_from(ast)?;
    Ok(arch.into_token_stream())
}

pub fn krate() -> Path {
    parse_quote!(lime_generic_def)
}
