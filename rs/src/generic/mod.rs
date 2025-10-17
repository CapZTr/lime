use eggmock::ReceiverFFI;
use lime_generic::{
    CompilerSettings, CompilerStatistics,
    copy::placeholder::CellOrVar,
    cost::{Cost, EqualCosts, OperationCost},
    definitions::{Ambit, AmbitCellType, FELIX, FELIXCellType, IMPLY, PLiM, SIMDRAM},
    generic_compiler_entrypoint,
    lime_generic_def::Instruction,
};

#[unsafe(no_mangle)]
pub extern "C" fn gp_compile_simdram<'a>(
    settings: CompilerSettings,
) -> ReceiverFFI<'a, CompilerStatistics> {
    let arch = SIMDRAM::new();
    ReceiverFFI::new(generic_compiler_entrypoint(
        arch, EqualCosts, settings, false,
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn gp_compile_ambit<'a>(
    settings: CompilerSettings,
) -> ReceiverFFI<'a, CompilerStatistics> {
    let arch = Ambit::new();
    ReceiverFFI::new(generic_compiler_entrypoint(
        arch, AmbitCost, settings, false,
    ))
}

#[derive(Clone)]
struct AmbitCost;

impl OperationCost<AmbitCellType> for AmbitCost {
    fn cost<I: Into<CellOrVar<AmbitCellType>>>(
        &self,
        instruction: &Instruction<I, AmbitCellType>,
    ) -> Cost {
        if instruction.outputs.is_empty() {
            (2.0 / 3.0).into()
        } else {
            1.into()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gp_compile_plim<'a>(
    settings: CompilerSettings,
) -> ReceiverFFI<'a, CompilerStatistics> {
    let arch = PLiM::new();
    ReceiverFFI::new(generic_compiler_entrypoint(
        arch, EqualCosts, settings, false,
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn gp_compile_imply<'a>(
    settings: CompilerSettings,
) -> ReceiverFFI<'a, CompilerStatistics> {
    let arch = IMPLY::new();
    ReceiverFFI::new(generic_compiler_entrypoint(
        arch, EqualCosts, settings, false,
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn gp_compile_felix<'a>(
    settings: CompilerSettings,
) -> ReceiverFFI<'a, CompilerStatistics> {
    let arch = FELIX::new();
    ReceiverFFI::new(generic_compiler_entrypoint(arch, FELIXCost, settings, true))
}

#[derive(Clone)]
struct FELIXCost;

impl OperationCost<FELIXCellType> for FELIXCost {
    fn cost<I: Into<CellOrVar<FELIXCellType>>>(
        &self,
        instruction: &Instruction<I, FELIXCellType>,
    ) -> Cost {
        if instruction.typ.id == FELIX::XOR_INSTRUCTION_ID {
            (1.5).into()
        } else {
            1.into()
        }
    }
}
