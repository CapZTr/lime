use super::define_generic_architecture;

define_generic_architecture! {
    pub Ambit {
        cells ([T; 4], [DCC; 2], [D]),
        operands (
            NONE = [()],
            DCC = [(DCC)],
            SRA_RD = [(bool), (T), (D), (DCC)],
            SRA_WR = [(T), (D), (DCC), (!DCC)],
            DRA = [
                (!DCC[0], T[0]),
                (!DCC[1], T[1]),
                (T[0], T[3]),
                (T[2], T[3]),
            ],
            TRA = [
                (T[0], T[1], T[2]),
                (T[1], T[2], T[3]),
                (DCC[0], T[1], T[2]),
                (DCC[1], T[0], T[3]),
            ],
        ),
        instructions (
            TRA = ([0..] := maj(TRA) -> (TRA, DRA, SRA_WR, NONE)),
            RC = (and(SRA_RD) -> (TRA, DRA, SRA_WR, NONE)),
            RC_INV = (and(DCC![0]) -> (TRA, DRA, SRA_WR, NONE)),
        ),
    }
}

define_generic_architecture! {
    pub SIMDRAM {
        cells ([T; 4], [DCC; 2], [D]),
        operands (
            NONE = [()],
            DCC = [(DCC)],
            SRA_RD = [(bool), (T), (D), (DCC)],
            SRA_WR = [(T), (D), (DCC), (!DCC)],
            DRA = [
                (!DCC[0], T[0]),
                (!DCC[1], T[1]),
                (T[2], T[3]),
                (T[0], T[3]),
            ],
            TRA = [
                (T[0], T[1], T[2]),
                (T[0], T[1], T[3]),
                (DCC[0], T[1], T[3]),
                (DCC[1], T[0], T[2]),
            ],
        ),
        instructions (
            TRA = ([0..] := maj(TRA) -> (TRA, DRA, SRA_WR, NONE)),
            RC = (and(SRA_RD) -> (TRA, DRA, SRA_WR, NONE)),
            RC_INV = (and(DCC![0]) -> (TRA, DRA, SRA_WR, NONE)),
        ),
    }
}

define_generic_architecture! {
    pub IMPLY {
        cells([D]),
        operands (
            PAIR = [(D | bool, D)],
            ANY = [(D)]
        ),
        instructions (
            IMP = ([1] := and(PAIR![1])),
            FALSE = ([0] := false(ANY))
        )
    }
}

define_generic_architecture! {
    pub PLiM {
        cells([D]),
        operands (
            TRIPLET = [
                (D | bool, D | bool, D),
            ]
        ),
        instructions (
            RM3 = ([2] := maj(TRIPLET![1]))
        )
    }
}

define_generic_architecture! {
    pub FELIX {
        cells([D]),
        operands (
            ANY = [(D)],
            NARY = (D | bool)*,
            TERNARY = [(D | bool, D | bool, D | bool)],
            BINARY = [(D | bool, D | bool)]
        ),
        instructions (
            // or
            OR = (!and(NARY![0..]) -> (ANY)),
            // nor
            NOR = (and(NARY![0..]) -> (ANY)),

            NAND2 = (!and(BINARY) -> (ANY)) ,
            NAND3 = (!and(TERNARY) -> (ANY)),
            MIN = (!maj(TERNARY) -> (ANY)),
            XOR = (xor(BINARY) -> (ANY)),
        )
    }
}
