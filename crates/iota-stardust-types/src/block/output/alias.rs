// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use alloc::{collections::BTreeSet, vec::Vec};

use packable::{Packable, bounded::BoundedU16, prefix::BoxedSlicePrefix};

use crate::block::{
    Error,
    address::{Address, AliasAddress},
    output::{
        AliasId, ChainId, NativeToken, NativeTokens, OutputBuilderAmount, OutputId,
        feature::{Feature, FeatureFlags, Features, verify_allowed_features},
        unlock_condition::{
            UnlockCondition, UnlockConditionFlags, UnlockConditions,
            verify_allowed_unlock_conditions,
        },
    },
};

/// Types of alias transition.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AliasTransition {
    /// State transition.
    State,
    /// Governance transition.
    Governance,
}

impl AliasTransition {
    /// Checks whether the alias transition is a state one.
    pub fn is_state(&self) -> bool {
        matches!(self, Self::State)
    }

    /// Checks whether the alias transition is a governance one.
    pub fn is_governance(&self) -> bool {
        matches!(self, Self::Governance)
    }
}

impl core::fmt::Display for AliasTransition {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::State => write!(f, "state"),
            Self::Governance => write!(f, "governance"),
        }
    }
}

///
#[derive(Clone)]
#[must_use]
pub struct AliasOutputBuilder {
    amount: OutputBuilderAmount,
    native_tokens: BTreeSet<NativeToken>,
    alias_id: AliasId,
    state_index: Option<u32>,
    state_metadata: Vec<u8>,
    foundry_counter: Option<u32>,
    unlock_conditions: BTreeSet<UnlockCondition>,
    features: BTreeSet<Feature>,
    immutable_features: BTreeSet<Feature>,
}

impl AliasOutputBuilder {
    /// Creates an [`AliasOutputBuilder`] with a provided amount.
    pub fn new_with_amount(amount: u64, alias_id: AliasId) -> Self {
        Self::new(OutputBuilderAmount::Amount(amount), alias_id)
    }

    fn new(amount: OutputBuilderAmount, alias_id: AliasId) -> Self {
        Self {
            amount,
            native_tokens: BTreeSet::new(),
            alias_id,
            state_index: None,
            state_metadata: Vec::new(),
            foundry_counter: None,
            unlock_conditions: BTreeSet::new(),
            features: BTreeSet::new(),
            immutable_features: BTreeSet::new(),
        }
    }

    /// Sets the amount to the provided value.
    #[inline(always)]
    pub fn with_amount(mut self, amount: u64) -> Self {
        self.amount = OutputBuilderAmount::Amount(amount);
        self
    }

    ///
    #[inline(always)]
    pub fn add_native_token(mut self, native_token: NativeToken) -> Self {
        self.native_tokens.insert(native_token);
        self
    }

    ///
    #[inline(always)]
    pub fn with_native_tokens(
        mut self,
        native_tokens: impl IntoIterator<Item = NativeToken>,
    ) -> Self {
        self.native_tokens = native_tokens.into_iter().collect();
        self
    }

    /// Sets the alias ID to the provided value.
    #[inline(always)]
    pub fn with_alias_id(mut self, alias_id: AliasId) -> Self {
        self.alias_id = alias_id;
        self
    }

    ///
    #[inline(always)]
    pub fn with_state_index(mut self, state_index: impl Into<Option<u32>>) -> Self {
        self.state_index = state_index.into();
        self
    }

    ///
    #[inline(always)]
    pub fn with_state_metadata(mut self, state_metadata: impl Into<Vec<u8>>) -> Self {
        self.state_metadata = state_metadata.into();
        self
    }

    ///
    #[inline(always)]
    pub fn with_foundry_counter(mut self, foundry_counter: impl Into<Option<u32>>) -> Self {
        self.foundry_counter = foundry_counter.into();
        self
    }

    /// Adds an [`UnlockCondition`] to the builder, if one does not already
    /// exist of that type.
    #[inline(always)]
    pub fn add_unlock_condition(mut self, unlock_condition: impl Into<UnlockCondition>) -> Self {
        self.unlock_conditions.insert(unlock_condition.into());
        self
    }

    /// Sets the [`UnlockConditions`]s in the builder, overwriting any existing
    /// values.
    #[inline(always)]
    pub fn with_unlock_conditions(
        mut self,
        unlock_conditions: impl IntoIterator<Item = impl Into<UnlockCondition>>,
    ) -> Self {
        self.unlock_conditions = unlock_conditions.into_iter().map(Into::into).collect();
        self
    }

    /// Replaces an [`UnlockCondition`] of the builder with a new one, or adds
    /// it.
    pub fn replace_unlock_condition(
        mut self,
        unlock_condition: impl Into<UnlockCondition>,
    ) -> Self {
        self.unlock_conditions.replace(unlock_condition.into());
        self
    }

    /// Clears all [`UnlockConditions`]s from the builder.
    #[inline(always)]
    pub fn clear_unlock_conditions(mut self) -> Self {
        self.unlock_conditions.clear();
        self
    }

    /// Adds a [`Feature`] to the builder, if one does not already exist of that
    /// type.
    #[inline(always)]
    pub fn add_feature(mut self, feature: impl Into<Feature>) -> Self {
        self.features.insert(feature.into());
        self
    }

    /// Sets the [`Feature`]s in the builder, overwriting any existing values.
    #[inline(always)]
    pub fn with_features(mut self, features: impl IntoIterator<Item = impl Into<Feature>>) -> Self {
        self.features = features.into_iter().map(Into::into).collect();
        self
    }

    /// Replaces a [`Feature`] of the builder with a new one, or adds it.
    pub fn replace_feature(mut self, feature: impl Into<Feature>) -> Self {
        self.features.replace(feature.into());
        self
    }

    /// Clears all [`Feature`]s from the builder.
    #[inline(always)]
    pub fn clear_features(mut self) -> Self {
        self.features.clear();
        self
    }

    /// Adds an immutable [`Feature`] to the builder, if one does not already
    /// exist of that type.
    #[inline(always)]
    pub fn add_immutable_feature(mut self, immutable_feature: impl Into<Feature>) -> Self {
        self.immutable_features.insert(immutable_feature.into());
        self
    }

    /// Sets the immutable [`Feature`]s in the builder, overwriting any existing
    /// values.
    #[inline(always)]
    pub fn with_immutable_features(
        mut self,
        immutable_features: impl IntoIterator<Item = impl Into<Feature>>,
    ) -> Self {
        self.immutable_features = immutable_features.into_iter().map(Into::into).collect();
        self
    }

    /// Replaces an immutable [`Feature`] of the builder with a new one, or adds
    /// it.
    pub fn replace_immutable_feature(mut self, immutable_feature: impl Into<Feature>) -> Self {
        self.immutable_features.replace(immutable_feature.into());
        self
    }

    /// Clears all immutable [`Feature`]s from the builder.
    #[inline(always)]
    pub fn clear_immutable_features(mut self) -> Self {
        self.immutable_features.clear();
        self
    }

    ///
    pub fn finish(self) -> Result<AliasOutput, Error> {
        let state_index = self.state_index.unwrap_or(0);
        let foundry_counter = self.foundry_counter.unwrap_or(0);

        let state_metadata = self
            .state_metadata
            .into_boxed_slice()
            .try_into()
            .map_err(Error::InvalidStateMetadataLength)?;

        verify_index_counter(&self.alias_id, state_index, foundry_counter)?;

        let unlock_conditions = UnlockConditions::from_set(self.unlock_conditions)?;

        verify_unlock_conditions(&unlock_conditions, &self.alias_id)?;

        let features = Features::from_set(self.features)?;

        verify_allowed_features(&features, AliasOutput::ALLOWED_FEATURES)?;

        let immutable_features = Features::from_set(self.immutable_features)?;

        verify_allowed_features(&immutable_features, AliasOutput::ALLOWED_IMMUTABLE_FEATURES)?;

        let mut output = AliasOutput {
            amount: 1,
            native_tokens: NativeTokens::from_set(self.native_tokens)?,
            alias_id: self.alias_id,
            state_index,
            state_metadata,
            foundry_counter,
            unlock_conditions,
            features,
            immutable_features,
        };

        output.amount = match self.amount {
            OutputBuilderAmount::Amount(amount) => amount,
        };

        Ok(output)
    }
}

impl From<&AliasOutput> for AliasOutputBuilder {
    fn from(output: &AliasOutput) -> Self {
        Self {
            amount: OutputBuilderAmount::Amount(output.amount),
            native_tokens: output.native_tokens.iter().copied().collect(),
            alias_id: output.alias_id,
            state_index: Some(output.state_index),
            state_metadata: output.state_metadata.to_vec(),
            foundry_counter: Some(output.foundry_counter),
            unlock_conditions: output.unlock_conditions.iter().cloned().collect(),
            features: output.features.iter().cloned().collect(),
            immutable_features: output.immutable_features.iter().cloned().collect(),
        }
    }
}

pub(crate) type StateMetadataLength = BoundedU16<0, { AliasOutput::STATE_METADATA_LENGTH_MAX }>;

/// Describes an alias account in the ledger that can be controlled by the state
/// and governance controllers.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Packable)]
#[packable(unpack_error = Error)]
pub struct AliasOutput {
    // Amount of IOTA tokens held by the output.
    amount: u64,
    // Native tokens held by the output.
    native_tokens: NativeTokens,
    // Unique identifier of the alias.
    alias_id: AliasId,
    // A counter that must increase by 1 every time the alias is state transitioned.
    state_index: u32,
    // Metadata that can only be changed by the state controller.
    #[packable(unpack_error_with = |err| Error::InvalidStateMetadataLength(err.into_prefix_err().into()))]
    state_metadata: BoxedSlicePrefix<u8, StateMetadataLength>,
    // A counter that denotes the number of foundries created by this alias account.
    foundry_counter: u32,
    unlock_conditions: UnlockConditions,
    //
    features: Features,
    //
    immutable_features: Features,
}

impl AliasOutput {
    /// The [`super::Output`] kind of an [`AliasOutput`].
    pub const KIND: u8 = 4;
    /// Maximum possible length in bytes of the state metadata.
    pub const STATE_METADATA_LENGTH_MAX: u16 = 8192;
    /// The set of allowed [`UnlockCondition`]s for an [`AliasOutput`].
    pub const ALLOWED_UNLOCK_CONDITIONS: UnlockConditionFlags =
        UnlockConditionFlags::STATE_CONTROLLER_ADDRESS
            .union(UnlockConditionFlags::GOVERNOR_ADDRESS);
    /// The set of allowed [`Feature`]s for an [`AliasOutput`].
    pub const ALLOWED_FEATURES: FeatureFlags = FeatureFlags::SENDER.union(FeatureFlags::METADATA);
    /// The set of allowed immutable [`Feature`]s for an [`AliasOutput`].
    pub const ALLOWED_IMMUTABLE_FEATURES: FeatureFlags =
        FeatureFlags::ISSUER.union(FeatureFlags::METADATA);

    /// Creates a new [`AliasOutputBuilder`] with a provided amount.
    #[inline(always)]
    pub fn build_with_amount(amount: u64, alias_id: AliasId) -> AliasOutputBuilder {
        AliasOutputBuilder::new_with_amount(amount, alias_id)
    }

    ///
    #[inline(always)]
    pub fn amount(&self) -> u64 {
        self.amount
    }

    ///
    #[inline(always)]
    pub fn native_tokens(&self) -> &NativeTokens {
        &self.native_tokens
    }

    ///
    #[inline(always)]
    pub fn alias_id(&self) -> &AliasId {
        &self.alias_id
    }

    /// Returns the alias ID if not null, or creates it from the output ID.
    #[inline(always)]
    pub fn alias_id_non_null(&self, output_id: &OutputId) -> AliasId {
        self.alias_id.or_from_output_id(output_id)
    }

    ///
    #[inline(always)]
    pub fn state_index(&self) -> u32 {
        self.state_index
    }

    ///
    #[inline(always)]
    pub fn state_metadata(&self) -> &[u8] {
        &self.state_metadata
    }

    ///
    #[inline(always)]
    pub fn foundry_counter(&self) -> u32 {
        self.foundry_counter
    }

    ///
    #[inline(always)]
    pub fn unlock_conditions(&self) -> &UnlockConditions {
        &self.unlock_conditions
    }

    ///
    #[inline(always)]
    pub fn features(&self) -> &Features {
        &self.features
    }

    ///
    #[inline(always)]
    pub fn immutable_features(&self) -> &Features {
        &self.immutable_features
    }

    ///
    #[inline(always)]
    pub fn state_controller_address(&self) -> &Address {
        // An AliasOutput must have a StateControllerAddressUnlockCondition.
        self.unlock_conditions
            .state_controller_address()
            .map(|unlock_condition| unlock_condition.address())
            .unwrap()
    }

    ///
    #[inline(always)]
    pub fn governor_address(&self) -> &Address {
        // An AliasOutput must have a GovernorAddressUnlockCondition.
        self.unlock_conditions
            .governor_address()
            .map(|unlock_condition| unlock_condition.address())
            .unwrap()
    }

    ///
    #[inline(always)]
    pub fn chain_id(&self) -> ChainId {
        ChainId::Alias(self.alias_id)
    }

    /// Returns the alias address for this output.
    pub fn alias_address(&self, output_id: &OutputId) -> AliasAddress {
        AliasAddress::new(self.alias_id_non_null(output_id))
    }
}

#[inline]
fn verify_index_counter(
    alias_id: &AliasId,
    state_index: u32,
    foundry_counter: u32,
) -> Result<(), Error> {
    if alias_id.is_null() && (state_index != 0 || foundry_counter != 0) {
        Err(Error::NonZeroStateIndexOrFoundryCounter)
    } else {
        Ok(())
    }
}

fn verify_unlock_conditions(
    unlock_conditions: &UnlockConditions,
    alias_id: &AliasId,
) -> Result<(), Error> {
    if let Some(unlock_condition) = unlock_conditions.state_controller_address() {
        if let Address::Alias(alias_address) = unlock_condition.address() {
            if !alias_id.is_null() && alias_address.alias_id() == alias_id {
                return Err(Error::SelfControlledAliasOutput(*alias_id));
            }
        }
    } else {
        return Err(Error::MissingStateControllerUnlockCondition);
    }

    if let Some(unlock_condition) = unlock_conditions.governor_address() {
        if let Address::Alias(alias_address) = unlock_condition.address() {
            if !alias_id.is_null() && alias_address.alias_id() == alias_id {
                return Err(Error::SelfControlledAliasOutput(*alias_id));
            }
        }
    } else {
        return Err(Error::MissingGovernorUnlockCondition);
    }

    verify_allowed_unlock_conditions(unlock_conditions, AliasOutput::ALLOWED_UNLOCK_CONDITIONS)
}
