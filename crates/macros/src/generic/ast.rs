use derive_more::{Deref, DerefMut};
use derive_syn_parse::Parse;
use lime_generic_def::InputIndices;
use proc_macro2::Span;
use syn::{
    Error, Ident, LitBool, LitInt, Result, Token, Visibility, bracketed,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Brace, Bracket, Paren},
};

#[derive(Debug, Parse)]
pub struct Architecture {
    pub vis: Visibility,
    pub name: Ident,
    #[expect(unused)]
    #[brace]
    pub brace: Brace,
    #[inside(brace)]
    pub inner: ArchitectureInner,
}

#[derive(Debug)]
pub struct ArchitectureInner {
    pub cells: Tuple<CellDef>,
    pub operands: Tuple<NameAndOpTuplesDef>,
    pub instructions: Tuple<Instruction>,
}

impl Parse for ArchitectureInner {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut cells = None;
        let mut operands = None;
        let mut instructions = None;
        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            match ident.to_string().as_str() {
                "cells" => cells = Some(input.parse()?),
                "operands" => operands = Some(input.parse()?),
                "instructions" => instructions = Some(input.parse()?),
                _ => return Err(Error::new(ident.span(), "invalid property")),
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }
        match (cells, operands, instructions) {
            (Some(cells), Some(operands), Some(instructions)) => Ok(Self {
                cells,
                operands,
                instructions,
            }),
            _ => Err(Error::new(input.span(), "missing property")),
        }
    }
}

#[derive(Debug, Parse)]
pub struct CellDef {
    #[expect(unused)]
    #[bracket]
    pub bracket: Bracket,
    #[inside(bracket)]
    pub name: Ident,
    #[inside(bracket)]
    #[expect(unused)]
    pub semi: Option<Token![;]>,
    #[inside(bracket)]
    #[parse_if(semi.is_some())]
    pub num: Option<LitInt>,
}

#[derive(Debug, Parse)]
pub struct NameAndOpTuplesDef {
    pub name: Ident,
    #[expect(unused)]
    pub eq: Token![=],
    pub operands: OpTuplesDef,
}

#[derive(Debug, Parse)]
pub enum OpTuplesDef {
    #[peek(Paren, name = "nary operands")]
    Nary {
        #[paren]
        #[expect(dead_code)]
        paren: Paren,
        #[inside(paren)]
        types: OperandPats,
        #[expect(dead_code)]
        star: Token![*],
    },
    #[peek(Bracket, name = "tuple operands")]
    Tuples {
        #[bracket]
        #[expect(dead_code)]
        bracket: Bracket,
        #[inside(bracket)]
        #[call(Punctuated::parse_terminated)]
        tuples: Punctuated<OperandTuplesElement, Token![,]>,
    },
}

#[derive(Debug, Parse)]
pub enum OperandTuplesElement {
    #[peek(Paren, name = "operand tuple pattern")]
    Tuple(Tuple<OperandPats>),
    #[peek(Token![...], name = "reference to a different operand tuple set")]
    Ref {
        #[expect(dead_code)]
        dots: Token![...],
        name: Ident,
    },
}

pub type Tuple<T> = Parenthesized<ParsePunctuated<T, Token![,]>>;

#[derive(Debug, Parse)]
pub struct OperandPat {
    pub invert: Option<Token![!]>,
    pub name: BoolOrIdent,
    pub index: MaybeBracketed<LitInt>,
}

#[derive(Debug)]
pub struct OperandPats(pub Vec<OperandPat>);

impl Parse for OperandPats {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut types = Vec::new();
        types.push(input.parse()?);
        loop {
            if input.is_empty() || !input.peek(Token![|]) {
                break;
            }
            input.parse::<Token![|]>()?;
            types.push(input.parse()?);
        }
        Ok(Self(types))
    }
}

#[derive(Debug, Parse)]
pub struct Instruction {
    pub name: Ident,
    #[expect(unused)]
    pub eq: Token![=],
    #[expect(unused)]
    #[paren]
    pub paren: Paren,
    #[inside(paren)]
    #[peek(Bracket)]
    pub input_target_idx: Option<Range>,
    #[inside(paren)]
    #[parse_if(input_target_idx.is_some())]
    #[expect(unused)]
    pub assign_op1: Option<Token![:]>,
    #[inside(paren)]
    #[parse_if(input_target_idx.is_some())]
    #[expect(unused)]
    pub assign_op2: Option<Token![=]>,
    #[inside(paren)]
    pub function: Function,
    #[inside(paren)]
    pub input: Parenthesized<InstructionInput>,
    #[prefix(Option<Token![->]> as arrow in paren)]
    #[inside(paren)]
    #[parse_if(arrow.is_some())]
    pub output: Option<Tuple<Ident>>,
}

#[derive(Debug, Parse)]
pub struct InstructionInput {
    pub operands: Ident,
    #[prefix(Option<Token![!]> as inv)]
    #[parse_if(inv.is_some())]
    pub inverted_range: Option<Range>,
}

#[derive(Debug, Parse)]
pub struct Function {
    pub inverted: Option<Token![!]>,
    pub gate: BoolOrIdent,
    #[peek(Bracket)]
    pub forwarded: Option<Range>,
}

#[derive(Debug)]
pub enum Range {
    LeftOpen { bracket: Bracket, end: LitInt },
    RightOpen { bracket: Bracket, start: LitInt },
    Single { bracket: Bracket, idx: LitInt },
}

impl Range {
    pub fn span(&self) -> Span {
        match self {
            Self::LeftOpen { bracket, .. } => bracket.span.join(),
            Self::RightOpen { bracket, .. } => bracket.span.join(),
            Self::Single { bracket, .. } => bracket.span.join(),
        }
    }
}

impl Parse for Range {
    fn parse(input: ParseStream) -> Result<Self> {
        let stream;
        let bracket = bracketed!(stream in input);
        if stream.peek(LitInt) {
            let int = stream.parse()?;
            if stream.is_empty() {
                return Ok(Range::Single { bracket, idx: int });
            } else if stream.peek(Token![..]) {
                stream.parse::<Token![..]>()?;
                return Ok(Range::RightOpen {
                    bracket,
                    start: int,
                });
            }
        } else if stream.peek(Token![..]) {
            stream.parse::<Token![..]>()?;
            let int = stream.parse()?;
            return Ok(Range::LeftOpen { bracket, end: int });
        }
        return Err(Error::new(stream.span(), "expected [..i], [i..] or [i]"));
    }
}

#[derive(Debug, Parse)]
pub enum BoolOrIdent {
    #[peek(LitBool, name = "boolean")]
    Bool(LitBool),
    #[peek(Ident, name = "identifier")]
    Ident(Ident),
}

#[derive(Debug, Parse)]
pub struct Bracketed<T> {
    #[bracket]
    pub bracket: Bracket,
    #[inside(bracket)]
    pub value: T,
}

#[derive(Debug)]
pub struct MaybeBracketed<T>(pub Option<Bracketed<T>>);

impl<T> Parse for MaybeBracketed<T>
where
    T: Parse,
{
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self(if input.peek(Bracket) {
            Some(input.parse()?)
        } else {
            None
        }))
    }
}

#[derive(Debug, Parse)]
pub struct Parenthesized<T> {
    #[paren]
    pub paren: Paren,
    #[inside(paren)]
    pub value: T,
}

#[derive(Debug, Deref, DerefMut)]
pub struct ParsePunctuated<T, P>(Punctuated<T, P>);

impl<T, P> Parse for ParsePunctuated<T, P>
where
    T: Parse,
    P: Parse,
{
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self(Punctuated::parse_terminated(input)?))
    }
}

impl TryFrom<&Range> for InputIndices {
    type Error = Error;

    fn try_from(value: &Range) -> Result<Self> {
        match value {
            Range::RightOpen { start, .. } if start.base10_parse::<i32>()? == 0 => {
                Ok(InputIndices::All)
            }
            Range::LeftOpen { end, .. } if end.base10_parse::<i32>()? == 0 => {
                Ok(InputIndices::None)
            }
            Range::Single { idx, .. } => Ok(InputIndices::Index(idx.base10_parse()?)),
            _ => Err(Error::new(
                value.span(),
                "this range is not supported here (yet)",
            )),
        }
    }
}

impl TryFrom<&Range> for lime_generic_def::Range {
    type Error = Error;

    fn try_from(value: &Range) -> Result<Self> {
        match value {
            Range::RightOpen { start, .. } => Ok(lime_generic_def::Range {
                start: start.base10_parse()?,
            }),
            _ => Err(Error::new(
                value.span(),
                "this range is not supported here (yet)",
            )),
        }
    }
}
