// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

//! This module defines the abstract state for the local safety analysis.

use move_binary_format::{
    binary_views::{BinaryIndexedView, FunctionView},
    errors::{PartialVMError, PartialVMResult},
    file_format::{AbilitySet, CodeOffset, FunctionDefinitionIndex, LocalIndex},
};
use move_core_types::vm_status::StatusCode;

use crate::absint::{AbstractDomain, JoinResult};

/// LocalState represents the current assignment state of a local
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LocalState {
    /// The local does not have a value
    Unavailable,
    /// The local was assigned a non-drop value in at least one control flow
    /// path,
    // but was `Unavailable` in at least one other path
    MaybeAvailable,
    /// The local has a value
    Available,
}
use LocalState::*;

use crate::meter::{Meter, Scope};

pub(crate) const STEP_BASE_COST: u128 = 15;
pub(crate) const RET_PER_LOCAL_COST: u128 = 30;
pub(crate) const JOIN_BASE_COST: u128 = 10;
pub(crate) const JOIN_PER_LOCAL_COST: u128 = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AbstractState {
    current_function: Option<FunctionDefinitionIndex>,
    all_local_abilities: Vec<AbilitySet>,
    local_states: Vec<LocalState>,
}

impl AbstractState {
    /// create a new abstract state
    pub fn new(
        resolver: &BinaryIndexedView,
        function_view: &FunctionView,
    ) -> PartialVMResult<Self> {
        let num_args = function_view.parameters().len();
        let num_locals = num_args + function_view.locals().len();
        let local_states = (0..num_locals)
            .map(|i| if i < num_args { Available } else { Unavailable })
            .collect();

        let all_local_abilities = function_view
            .parameters()
            .0
            .iter()
            .chain(function_view.locals().0.iter())
            .map(|st| resolver.abilities(st, function_view.type_parameters()))
            .collect::<PartialVMResult<Vec<_>>>()?;

        Ok(Self {
            current_function: function_view.index(),
            local_states,
            all_local_abilities,
        })
    }

    pub fn local_abilities(&self, idx: LocalIndex) -> AbilitySet {
        self.all_local_abilities[idx as usize]
    }

    pub fn all_local_abilities(&self) -> &Vec<AbilitySet> {
        &self.all_local_abilities
    }

    pub fn local_state(&self, idx: LocalIndex) -> LocalState {
        self.local_states[idx as usize]
    }

    pub fn local_states(&self) -> &Vec<LocalState> {
        &self.local_states
    }

    pub fn set_available(&mut self, idx: LocalIndex) {
        self.local_states[idx as usize] = Available
    }

    pub fn set_unavailable(&mut self, idx: LocalIndex) {
        assert!(self.local_states[idx as usize] == Available);
        self.local_states[idx as usize] = Unavailable
    }

    pub fn error(&self, status: StatusCode, offset: CodeOffset) -> PartialVMError {
        PartialVMError::new(status).at_code_offset(
            self.current_function.unwrap_or(FunctionDefinitionIndex(0)),
            offset,
        )
    }

    fn join_(&self, other: &Self) -> Self {
        assert!(self.current_function == other.current_function);
        assert!(self.all_local_abilities.len() == other.all_local_abilities.len());
        assert!(self.local_states.len() == other.local_states.len());
        let current_function = self.current_function;
        let all_local_abilities = self.all_local_abilities.clone();
        let local_states = self
            .local_states
            .iter()
            .zip(&other.local_states)
            .map(|(self_state, other_state)| {
                use LocalState::*;
                match (self_state, other_state) {
                    // Unavailable on both sides, nothing to add
                    (Unavailable, Unavailable) => Unavailable,

                    (MaybeAvailable, Unavailable)
                    | (Unavailable, MaybeAvailable)
                    | (MaybeAvailable, MaybeAvailable)
                    | (MaybeAvailable, Available)
                    | (Available, MaybeAvailable)
                    | (Unavailable, Available)
                    | (Available, Unavailable) => MaybeAvailable,

                    (Available, Available) => Available,
                }
            })
            .collect();

        Self {
            current_function,
            all_local_abilities,
            local_states,
        }
    }
}

impl AbstractDomain for AbstractState {
    /// attempts to join state to self and returns the result
    fn join(
        &mut self,
        state: &AbstractState,
        meter: &mut impl Meter,
    ) -> PartialVMResult<JoinResult> {
        meter.add(Scope::Function, JOIN_BASE_COST)?;
        meter.add_items(
            Scope::Function,
            JOIN_PER_LOCAL_COST,
            state.local_states.len(),
        )?;
        let joined = Self::join_(self, state);
        assert!(self.local_states.len() == joined.local_states.len());
        let locals_unchanged = self
            .local_states
            .iter()
            .zip(&joined.local_states)
            .all(|(self_state, other_state)| self_state == other_state);
        if locals_unchanged {
            Ok(JoinResult::Unchanged)
        } else {
            *self = joined;
            Ok(JoinResult::Changed)
        }
    }
}
