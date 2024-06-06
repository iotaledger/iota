// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeSet;

use move_ir_types::ast as IR;

use crate::parser::ast::FunctionName;

// Removes any unnecessary storing to a local just to move the value out.

#[allow(clippy::ptr_arg)]
pub fn optimize(
    _f: &FunctionName,
    _loop_heads: &BTreeSet<IR::BlockLabel_>,
    _locals: &mut Vec<(IR::Var, IR::Type)>,
    blocks: &mut IR::BytecodeBlocks,
) -> bool {
    let mut changed = false;
    for (_lbl, block) in blocks {
        let mut new_block = vec![];
        let mut i = 0;
        while i < block.len() {
            match (&block[i], block.get(i + 1)) {
                (sp!(_, IR::Bytecode_::StLoc(v1)), Some(sp!(_, IR::Bytecode_::MoveLoc(v2))))
                    if v1 == v2 =>
                {
                    changed = true;
                    i += 2
                }
                (instr, _) => {
                    new_block.push(instr.clone());
                    i += 1
                }
            }
        }
        *block = new_block;
    }

    changed
}
