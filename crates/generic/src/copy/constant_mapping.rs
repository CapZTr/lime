use lime_generic_def::{
    BoolHint, Cell, CellPat, CellType, Function, FunctionEvaluation, InputIndices, Range, set::Set,
};

#[derive(Debug)]
pub enum ConstantMappingHint {
    Value(bool),
    Identity { inverted: bool },
}

impl ConstantMappingHint {
    fn get(&self, eval: &FunctionEvaluation) -> Option<BoolHint> {
        match self {
            ConstantMappingHint::Identity { inverted } => eval.hint_id(*inverted),
            ConstantMappingHint::Value(value) => eval.hint(*value),
        }
    }
}

#[derive(derive_more::Debug)]
pub struct ConstantMapping<'t, CT> {
    pub function: Function,
    pub operands: &'t [CellPat<CT>],
    pub inverted: InputIndices,
    pub hint: ConstantMappingHint,
}

// TODO: handle ignore differently than this here mess
pub fn map_constants<CT, TargetCT>(
    function: Function,
    hint: ConstantMappingHint,
    inverted: InputIndices,
    operands: &[CellPat<CT>],
    range: Range,
    ignore: Option<usize>,
    ignore2: Option<usize>,
) -> Vec<(Vec<Cell<TargetCT>>, FunctionEvaluation)>
where
    TargetCT: CellType,
    CT: CellType + Into<TargetCT>,
{
    let (prepend, _, append) = range.slice(operands);
    if prepend != 0 || append != 0 {
        return vec![];
    }

    let clamp_ignore = |ignore: Option<usize>| {
        ignore.and_then(|ignore| {
            if ignore >= operands.len() {
                None
            } else {
                Some(ignore)
            }
        })
    };
    ConstantMapping {
        function,
        operands,
        inverted,
        hint,
    }
    .map_all(clamp_ignore(ignore), clamp_ignore(ignore2))
}

impl<'t, CT> ConstantMapping<'t, CT>
where
    CT: CellType,
{
    pub fn map_all<TargetCT>(
        &self,
        ignore: Option<usize>,
        ignore2: Option<usize>,
    ) -> Vec<(Vec<Cell<TargetCT>>, FunctionEvaluation)>
    where
        TargetCT: CellType,
        CT: CellType + Into<TargetCT>,
    {
        let num_ignore = ignore.is_some() as usize + ignore2.is_some() as usize;
        let ignore = |idx: usize| Some(idx) == ignore || Some(idx) == ignore2;
        let num_operands_without_ignored = self.operands.len() - num_ignore;
        let eval = self.function.evaluate(self.operands.len());
        let mut result = Vec::new();

        if num_operands_without_ignored == 0 {
            return vec![(vec![], eval)];
        } else if num_operands_without_ignored == 1 {
            let mut idx = 0;
            while ignore(idx) {
                idx += 1;
            }
            for (cell, eval) in self.try_match(eval, &self.operands[idx], idx, None) {
                result.push((vec![cell.map_cell_type(Into::into)], eval));
            }
            return result;
        }

        for (first_idx, first_cell_pat) in self.operands.iter().enumerate() {
            if ignore(first_idx) {
                continue;
            }
            for (second_idx, second_cell_pat) in self.operands.iter().enumerate() {
                if first_idx == second_idx || ignore(second_idx) {
                    continue;
                }

                for (first_cell, eval) in self.try_match(eval, first_cell_pat, first_idx, None) {
                    'outer: for (second_operand, eval) in
                        self.try_match(eval, second_cell_pat, second_idx, Some(first_cell))
                    {
                        let mut mapping = Vec::new();
                        for i in 0..self.operands.len() {
                            if i == first_idx {
                                mapping.push(first_cell.map_cell_type(Into::into));
                            } else if i == second_idx {
                                mapping.push(second_operand.map_cell_type(Into::into));
                            } else if !ignore(i) {
                                continue 'outer;
                            }
                        }
                        result.push((mapping, eval));
                    }
                }
            }
        }
        result
    }

    fn try_match(
        &self,
        eval: FunctionEvaluation,
        cell: &CellPat<CT>,
        index: usize,
        forbidden_cell: Option<Cell<CT>>,
    ) -> impl Iterator<Item = (Cell<CT>, FunctionEvaluation)> {
        let tf = &[true, false];
        let values = match self.hint.get(&eval) {
            None => &[],
            Some(BoolHint::Require(value)) => {
                if value ^ self.inverted.contains(&index) {
                    &tf[0..1]
                } else {
                    &tf[1..2]
                }
            }
            _ => tf,
        };

        values.iter().flat_map(move |value| {
            cell.get_constant(*value).into_iter().flat_map(move |cell| {
                if Some(cell) == forbidden_cell {
                    return None;
                }
                let mut new_eval = eval;
                new_eval.add(*value ^ self.inverted.contains(&index));
                Some((cell, new_eval))
            })
        })
    }
}
