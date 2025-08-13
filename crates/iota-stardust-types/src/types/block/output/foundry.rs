// Copyright 2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use alloc::collections::{BTreeMap, BTreeSet};
use core::cmp::Ordering;

use packable::{
    Packable,
    error::{UnpackError, UnpackErrorExt},
    packer::Packer,
    unpacker::Unpacker,
};
use primitive_types::U256;

use super::verify_output_amount_packable;
use crate::types::{
    ValidationParams,
    block::{
        Error,
        address::{Address, AliasAddress},
        output::{
            ChainId, FoundryId, NativeToken, NativeTokens, Output, OutputBuilderAmount, OutputId,
            Rent, RentStructure, StateTransitionError, StateTransitionVerifier, TokenId,
            TokenScheme,
            feature::{Feature, FeatureFlags, Features, verify_allowed_features},
            unlock_condition::{
                UnlockCondition, UnlockConditionFlags, UnlockConditions,
                verify_allowed_unlock_conditions,
            },
            verify_output_amount,
        },
        protocol::ProtocolParameters,
        semantic::{ConflictReason, ValidationContext},
        unlock::Unlock,
    },
};

///
#[derive(Clone)]
#[must_use]
pub struct FoundryOutputBuilder {
    amount: OutputBuilderAmount,
    native_tokens: BTreeSet<NativeToken>,
    serial_number: u32,
    token_scheme: TokenScheme,
    unlock_conditions: BTreeSet<UnlockCondition>,
    features: BTreeSet<Feature>,
    immutable_features: BTreeSet<Feature>,
}

impl FoundryOutputBuilder {
    /// Creates a [`FoundryOutputBuilder`] with a provided amount.
    pub fn new_with_amount(amount: u64, serial_number: u32, token_scheme: TokenScheme) -> Self {
        Self::new(
            OutputBuilderAmount::Amount(amount),
            serial_number,
            token_scheme,
        )
    }

    /// Creates a [`FoundryOutputBuilder`] with a provided rent structure.
    /// The amount will be set to the minimum storage deposit.
    pub fn new_with_minimum_storage_deposit(
        rent_structure: RentStructure,
        serial_number: u32,
        token_scheme: TokenScheme,
    ) -> Self {
        Self::new(
            OutputBuilderAmount::MinimumStorageDeposit(rent_structure),
            serial_number,
            token_scheme,
        )
    }

    fn new(amount: OutputBuilderAmount, serial_number: u32, token_scheme: TokenScheme) -> Self {
        Self {
            amount,
            native_tokens: BTreeSet::new(),
            serial_number,
            token_scheme,
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

    /// Sets the amount to the minimum storage deposit.
    #[inline(always)]
    pub fn with_minimum_storage_deposit(mut self, rent_structure: RentStructure) -> Self {
        self.amount = OutputBuilderAmount::MinimumStorageDeposit(rent_structure);
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

    /// Sets the serial number to the provided value.
    #[inline(always)]
    pub fn with_serial_number(mut self, serial_number: u32) -> Self {
        self.serial_number = serial_number;
        self
    }

    /// Sets the token scheme to the provided value.
    #[inline(always)]
    pub fn with_token_scheme(mut self, token_scheme: TokenScheme) -> Self {
        self.token_scheme = token_scheme;
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
    pub fn finish(self) -> Result<FoundryOutput, Error> {
        if self.serial_number == 0 {
            return Err(Error::InvalidFoundryZeroSerialNumber);
        }

        let unlock_conditions = UnlockConditions::from_set(self.unlock_conditions)?;

        verify_unlock_conditions(&unlock_conditions)?;

        let features = Features::from_set(self.features)?;

        verify_allowed_features(&features, FoundryOutput::ALLOWED_FEATURES)?;

        let immutable_features = Features::from_set(self.immutable_features)?;

        verify_allowed_features(
            &immutable_features,
            FoundryOutput::ALLOWED_IMMUTABLE_FEATURES,
        )?;

        let mut output = FoundryOutput {
            amount: 1u64,
            native_tokens: NativeTokens::from_set(self.native_tokens)?,
            serial_number: self.serial_number,
            token_scheme: self.token_scheme,
            unlock_conditions,
            features,
            immutable_features,
        };

        output.amount = match self.amount {
            OutputBuilderAmount::Amount(amount) => amount,
            OutputBuilderAmount::MinimumStorageDeposit(rent_structure) => {
                Output::Foundry(output.clone()).rent_cost(&rent_structure)
            }
        };

        Ok(output)
    }

    ///
    pub fn finish_with_params<'a>(
        self,
        params: impl Into<ValidationParams<'a>> + Send,
    ) -> Result<FoundryOutput, Error> {
        let output = self.finish()?;

        if let Some(token_supply) = params.into().token_supply() {
            verify_output_amount(&output.amount, &token_supply)?;
        }

        Ok(output)
    }

    /// Finishes the [`FoundryOutputBuilder`] into an [`Output`].
    pub fn finish_output<'a>(
        self,
        params: impl Into<ValidationParams<'a>> + Send,
    ) -> Result<Output, Error> {
        Ok(Output::Foundry(self.finish_with_params(params)?))
    }
}

impl From<&FoundryOutput> for FoundryOutputBuilder {
    fn from(output: &FoundryOutput) -> Self {
        Self {
            amount: OutputBuilderAmount::Amount(output.amount),
            native_tokens: output.native_tokens.iter().copied().collect(),
            serial_number: output.serial_number,
            token_scheme: output.token_scheme.clone(),
            unlock_conditions: output.unlock_conditions.iter().cloned().collect(),
            features: output.features.iter().cloned().collect(),
            immutable_features: output.immutable_features.iter().cloned().collect(),
        }
    }
}

/// Describes a foundry output that is controlled by an alias.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FoundryOutput {
    // Amount of IOTA tokens held by the output.
    amount: u64,
    // Native tokens held by the output.
    native_tokens: NativeTokens,
    // The serial number of the foundry with respect to the controlling alias.
    serial_number: u32,
    token_scheme: TokenScheme,
    unlock_conditions: UnlockConditions,
    features: Features,
    immutable_features: Features,
}

impl FoundryOutput {
    /// The [`Output`] kind of a [`FoundryOutput`].
    pub const KIND: u8 = 5;
    /// The set of allowed [`UnlockCondition`]s for a [`FoundryOutput`].
    pub const ALLOWED_UNLOCK_CONDITIONS: UnlockConditionFlags =
        UnlockConditionFlags::IMMUTABLE_ALIAS_ADDRESS;
    /// The set of allowed [`Feature`]s for a [`FoundryOutput`].
    pub const ALLOWED_FEATURES: FeatureFlags = FeatureFlags::METADATA;
    /// The set of allowed immutable [`Feature`]s for a [`FoundryOutput`].
    pub const ALLOWED_IMMUTABLE_FEATURES: FeatureFlags = FeatureFlags::METADATA;

    /// Creates a new [`FoundryOutputBuilder`] with a provided amount.
    #[inline(always)]
    pub fn build_with_amount(
        amount: u64,
        serial_number: u32,
        token_scheme: TokenScheme,
    ) -> FoundryOutputBuilder {
        FoundryOutputBuilder::new_with_amount(amount, serial_number, token_scheme)
    }

    /// Creates a new [`FoundryOutputBuilder`] with a provided rent structure.
    /// The amount will be set to the minimum storage deposit.
    #[inline(always)]
    pub fn build_with_minimum_storage_deposit(
        rent_structure: RentStructure,
        serial_number: u32,
        token_scheme: TokenScheme,
    ) -> FoundryOutputBuilder {
        FoundryOutputBuilder::new_with_minimum_storage_deposit(
            rent_structure,
            serial_number,
            token_scheme,
        )
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
    pub fn serial_number(&self) -> u32 {
        self.serial_number
    }

    ///
    #[inline(always)]
    pub fn token_scheme(&self) -> &TokenScheme {
        &self.token_scheme
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
    pub fn alias_address(&self) -> &AliasAddress {
        // A FoundryOutput must have an ImmutableAliasAddressUnlockCondition.
        self.unlock_conditions
            .immutable_alias_address()
            .map(|unlock_condition| unlock_condition.alias_address())
            .unwrap()
    }

    /// Returns the [`FoundryId`] of the [`FoundryOutput`].
    pub fn id(&self) -> FoundryId {
        FoundryId::build(
            self.alias_address(),
            self.serial_number,
            self.token_scheme.kind(),
        )
    }

    /// Returns the [`TokenId`] of the [`FoundryOutput`].
    pub fn token_id(&self) -> TokenId {
        TokenId::from(self.id())
    }

    ///
    #[inline(always)]
    pub fn chain_id(&self) -> ChainId {
        ChainId::Foundry(self.id())
    }

    ///
    pub fn unlock(
        &self,
        _output_id: &OutputId,
        unlock: &Unlock,
        inputs: &[(&OutputId, &Output)],
        context: &mut ValidationContext<'_>,
    ) -> Result<(), ConflictReason> {
        Address::from(*self.alias_address()).unlock(unlock, inputs, context)
    }

    // Transition, just without full ValidationContext
    pub(crate) fn transition_inner(
        current_state: &Self,
        next_state: &Self,
        input_native_tokens: &BTreeMap<TokenId, U256>,
        output_native_tokens: &BTreeMap<TokenId, U256>,
    ) -> Result<(), StateTransitionError> {
        if current_state.alias_address() != next_state.alias_address()
            || current_state.serial_number != next_state.serial_number
            || current_state.immutable_features != next_state.immutable_features
        {
            return Err(StateTransitionError::MutatedImmutableField);
        }

        let token_id = next_state.token_id();
        let input_tokens = input_native_tokens
            .get(&token_id)
            .copied()
            .unwrap_or_default();
        let output_tokens = output_native_tokens
            .get(&token_id)
            .copied()
            .unwrap_or_default();
        let TokenScheme::Simple(ref current_token_scheme) = current_state.token_scheme;
        let TokenScheme::Simple(ref next_token_scheme) = next_state.token_scheme;

        if current_token_scheme.maximum_supply() != next_token_scheme.maximum_supply() {
            return Err(StateTransitionError::MutatedImmutableField);
        }

        if current_token_scheme.minted_tokens() > next_token_scheme.minted_tokens()
            || current_token_scheme.melted_tokens() > next_token_scheme.melted_tokens()
        {
            return Err(StateTransitionError::NonMonotonicallyIncreasingNativeTokens);
        }

        match input_tokens.cmp(&output_tokens) {
            Ordering::Less => {
                // Mint

                // This can't underflow as it is known that current_minted_tokens <=
                // next_minted_tokens.
                let minted_diff =
                    next_token_scheme.minted_tokens() - current_token_scheme.minted_tokens();
                // This can't underflow as it is known that input_tokens < output_tokens
                // (Ordering::Less).
                let token_diff = output_tokens - input_tokens;

                if minted_diff != token_diff {
                    return Err(StateTransitionError::InconsistentNativeTokensMint);
                }

                if current_token_scheme.melted_tokens() != next_token_scheme.melted_tokens() {
                    return Err(StateTransitionError::InconsistentNativeTokensMint);
                }
            }
            Ordering::Equal => {
                // Transition

                if current_token_scheme.minted_tokens() != next_token_scheme.minted_tokens()
                    || current_token_scheme.melted_tokens() != next_token_scheme.melted_tokens()
                {
                    return Err(StateTransitionError::InconsistentNativeTokensTransition);
                }
            }
            Ordering::Greater => {
                // Melt / Burn

                if current_token_scheme.melted_tokens() != next_token_scheme.melted_tokens()
                    && current_token_scheme.minted_tokens() != next_token_scheme.minted_tokens()
                {
                    return Err(StateTransitionError::InconsistentNativeTokensMeltBurn);
                }

                // This can't underflow as it is known that current_melted_tokens <=
                // next_melted_tokens.
                let melted_diff =
                    next_token_scheme.melted_tokens() - current_token_scheme.melted_tokens();
                // This can't underflow as it is known that input_tokens > output_tokens
                // (Ordering::Greater).
                let token_diff = input_tokens - output_tokens;

                if melted_diff > token_diff {
                    return Err(StateTransitionError::InconsistentNativeTokensMeltBurn);
                }
            }
        }

        Ok(())
    }
}

impl StateTransitionVerifier for FoundryOutput {
    fn creation(
        next_state: &Self,
        context: &ValidationContext<'_>,
    ) -> Result<(), StateTransitionError> {
        let alias_chain_id = ChainId::from(*next_state.alias_address().alias_id());

        if let (Some(Output::Alias(input_alias)), Some(Output::Alias(output_alias))) = (
            context.input_chains.get(&alias_chain_id),
            context.output_chains.get(&alias_chain_id),
        ) {
            if input_alias.foundry_counter() >= next_state.serial_number()
                || next_state.serial_number() > output_alias.foundry_counter()
            {
                return Err(StateTransitionError::InconsistentFoundrySerialNumber);
            }
        } else {
            return Err(StateTransitionError::MissingAliasForFoundry);
        }

        let token_id = next_state.token_id();
        let output_tokens = context
            .output_native_tokens
            .get(&token_id)
            .copied()
            .unwrap_or_default();
        let TokenScheme::Simple(ref next_token_scheme) = next_state.token_scheme;

        // No native tokens should be referenced prior to the foundry creation.
        if context.input_native_tokens.contains_key(&token_id) {
            return Err(StateTransitionError::InconsistentNativeTokensFoundryCreation);
        }

        if output_tokens != next_token_scheme.minted_tokens()
            || !next_token_scheme.melted_tokens().is_zero()
        {
            return Err(StateTransitionError::InconsistentNativeTokensFoundryCreation);
        }

        Ok(())
    }

    fn transition(
        current_state: &Self,
        next_state: &Self,
        context: &ValidationContext<'_>,
    ) -> Result<(), StateTransitionError> {
        Self::transition_inner(
            current_state,
            next_state,
            &context.input_native_tokens,
            &context.output_native_tokens,
        )
    }

    fn destruction(
        current_state: &Self,
        context: &ValidationContext<'_>,
    ) -> Result<(), StateTransitionError> {
        let token_id = current_state.token_id();
        let input_tokens = context
            .input_native_tokens
            .get(&token_id)
            .copied()
            .unwrap_or_default();
        let TokenScheme::Simple(ref current_token_scheme) = current_state.token_scheme;

        // No native tokens should be referenced after the foundry destruction.
        if context.output_native_tokens.contains_key(&token_id) {
            return Err(StateTransitionError::InconsistentNativeTokensFoundryDestruction);
        }

        // This can't underflow as it is known that minted_tokens >= melted_tokens
        // (syntactic rule).
        let minted_melted_diff =
            current_token_scheme.minted_tokens() - current_token_scheme.melted_tokens();

        if minted_melted_diff != input_tokens {
            return Err(StateTransitionError::InconsistentNativeTokensFoundryDestruction);
        }

        Ok(())
    }
}

impl Packable for FoundryOutput {
    type UnpackError = Error;
    type UnpackVisitor = ProtocolParameters;

    fn pack<P: Packer>(&self, packer: &mut P) -> Result<(), P::Error> {
        self.amount.pack(packer)?;
        self.native_tokens.pack(packer)?;
        self.serial_number.pack(packer)?;
        self.token_scheme.pack(packer)?;
        self.unlock_conditions.pack(packer)?;
        self.features.pack(packer)?;
        self.immutable_features.pack(packer)?;

        Ok(())
    }

    fn unpack<U: Unpacker, const VERIFY: bool>(
        unpacker: &mut U,
        visitor: &Self::UnpackVisitor,
    ) -> Result<Self, UnpackError<Self::UnpackError, U::Error>> {
        let amount = u64::unpack::<_, VERIFY>(unpacker, &()).coerce()?;

        verify_output_amount_packable::<VERIFY>(&amount, visitor).map_err(UnpackError::Packable)?;

        let native_tokens = NativeTokens::unpack::<_, VERIFY>(unpacker, &())?;
        let serial_number = u32::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let token_scheme = TokenScheme::unpack::<_, VERIFY>(unpacker, &())?;

        let unlock_conditions = UnlockConditions::unpack::<_, VERIFY>(unpacker, visitor)?;

        if VERIFY {
            verify_unlock_conditions(&unlock_conditions).map_err(UnpackError::Packable)?;
        }

        let features = Features::unpack::<_, VERIFY>(unpacker, &())?;

        if VERIFY {
            verify_allowed_features(&features, Self::ALLOWED_FEATURES)
                .map_err(UnpackError::Packable)?;
        }

        let immutable_features = Features::unpack::<_, VERIFY>(unpacker, &())?;

        if VERIFY {
            verify_allowed_features(&immutable_features, Self::ALLOWED_IMMUTABLE_FEATURES)
                .map_err(UnpackError::Packable)?;
        }

        Ok(Self {
            amount,
            native_tokens,
            serial_number,
            token_scheme,
            unlock_conditions,
            features,
            immutable_features,
        })
    }
}

fn verify_unlock_conditions(unlock_conditions: &UnlockConditions) -> Result<(), Error> {
    if unlock_conditions.immutable_alias_address().is_none() {
        Err(Error::MissingAddressUnlockCondition)
    } else {
        verify_allowed_unlock_conditions(
            unlock_conditions,
            FoundryOutput::ALLOWED_UNLOCK_CONDITIONS,
        )
    }
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::{
        string::{String, ToString},
        vec::Vec,
    };

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::{
        TryFromDto,
        block::{
            Error,
            output::{
                dto::OutputBuilderAmountDto, feature::dto::FeatureDto,
                token_scheme::dto::TokenSchemeDto, unlock_condition::dto::UnlockConditionDto,
            },
        },
    };

    /// Describes a foundry output that is controlled by an alias.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct FoundryOutputDto {
        #[serde(rename = "type")]
        pub kind: u8,
        // Amount of IOTA tokens held by the output.
        pub amount: String,
        // Native tokens held by the output.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub native_tokens: Vec<NativeToken>,
        // The serial number of the foundry with respect to the controlling alias.
        pub serial_number: u32,
        pub token_scheme: TokenSchemeDto,
        pub unlock_conditions: Vec<UnlockConditionDto>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub features: Vec<FeatureDto>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub immutable_features: Vec<FeatureDto>,
    }

    impl From<&FoundryOutput> for FoundryOutputDto {
        fn from(value: &FoundryOutput) -> Self {
            Self {
                kind: FoundryOutput::KIND,
                amount: value.amount().to_string(),
                native_tokens: value.native_tokens().to_vec(),
                serial_number: value.serial_number(),
                token_scheme: value.token_scheme().into(),
                unlock_conditions: value
                    .unlock_conditions()
                    .iter()
                    .map(Into::into)
                    .collect::<_>(),
                features: value.features().iter().map(Into::into).collect::<_>(),
                immutable_features: value
                    .immutable_features()
                    .iter()
                    .map(Into::into)
                    .collect::<_>(),
            }
        }
    }

    impl TryFromDto for FoundryOutput {
        type Dto = FoundryOutputDto;
        type Error = Error;

        fn try_from_dto_with_params_inner(
            dto: Self::Dto,
            params: ValidationParams<'_>,
        ) -> Result<Self, Self::Error> {
            let mut builder = FoundryOutputBuilder::new_with_amount(
                dto.amount
                    .parse::<u64>()
                    .map_err(|_| Error::InvalidField("amount"))?,
                dto.serial_number,
                dto.token_scheme.try_into()?,
            );

            for t in dto.native_tokens {
                builder = builder.add_native_token(t);
            }

            for b in dto.features {
                builder = builder.add_feature(Feature::try_from(b)?);
            }

            for b in dto.immutable_features {
                builder = builder.add_immutable_feature(Feature::try_from(b)?);
            }

            for u in dto.unlock_conditions {
                builder = builder
                    .add_unlock_condition(UnlockCondition::try_from_dto_with_params(u, &params)?);
            }

            builder.finish_with_params(params)
        }
    }

    impl FoundryOutput {
        #[allow(clippy::too_many_arguments)]
        pub fn try_from_dtos<'a>(
            amount: OutputBuilderAmountDto,
            native_tokens: Option<Vec<NativeToken>>,
            serial_number: u32,
            token_scheme: TokenSchemeDto,
            unlock_conditions: Vec<UnlockConditionDto>,
            features: Option<Vec<FeatureDto>>,
            immutable_features: Option<Vec<FeatureDto>>,
            params: impl Into<ValidationParams<'a>> + Send,
        ) -> Result<Self, Error> {
            let params = params.into();
            let token_scheme = TokenScheme::try_from(token_scheme)?;

            let mut builder = match amount {
                OutputBuilderAmountDto::Amount(amount) => FoundryOutputBuilder::new_with_amount(
                    amount.parse().map_err(|_| Error::InvalidField("amount"))?,
                    serial_number,
                    token_scheme,
                ),
                OutputBuilderAmountDto::MinimumStorageDeposit(rent_structure) => {
                    FoundryOutputBuilder::new_with_minimum_storage_deposit(
                        rent_structure,
                        serial_number,
                        token_scheme,
                    )
                }
            };

            if let Some(native_tokens) = native_tokens {
                builder = builder.with_native_tokens(native_tokens);
            }

            let unlock_conditions = unlock_conditions
                .into_iter()
                .map(|u| UnlockCondition::try_from_dto_with_params(u, &params))
                .collect::<Result<Vec<UnlockCondition>, Error>>()?;
            builder = builder.with_unlock_conditions(unlock_conditions);

            if let Some(features) = features {
                let features = features
                    .into_iter()
                    .map(Feature::try_from)
                    .collect::<Result<Vec<Feature>, Error>>()?;
                builder = builder.with_features(features);
            }

            if let Some(immutable_features) = immutable_features {
                let immutable_features = immutable_features
                    .into_iter()
                    .map(Feature::try_from)
                    .collect::<Result<Vec<Feature>, Error>>()?;
                builder = builder.with_immutable_features(immutable_features);
            }

            builder.finish_with_params(params)
        }
    }
}

#[cfg(test)]
mod tests {
    use packable::PackableExt;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::types::{
        TryFromDto,
        block::{
            output::{
                FoundryId, SimpleTokenScheme, TokenId, dto::OutputDto,
                unlock_condition::ImmutableAliasAddressUnlockCondition,
            },
            protocol::protocol_parameters,
            rand::{
                address::rand_alias_address,
                output::{
                    feature::{rand_allowed_features, rand_metadata_feature},
                    rand_foundry_output, rand_token_scheme,
                },
            },
        },
    };

    #[test]
    fn builder() {
        let protocol_parameters = protocol_parameters();
        let foundry_id = FoundryId::build(&rand_alias_address(), 0, SimpleTokenScheme::KIND);
        let alias_1 = ImmutableAliasAddressUnlockCondition::new(rand_alias_address());
        let alias_2 = ImmutableAliasAddressUnlockCondition::new(rand_alias_address());
        let metadata_1 = rand_metadata_feature();
        let metadata_2 = rand_metadata_feature();

        let mut builder = FoundryOutput::build_with_amount(0, 234, rand_token_scheme())
            .with_serial_number(85)
            .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
            .with_unlock_conditions([alias_1])
            .add_feature(metadata_1.clone())
            .replace_feature(metadata_2.clone())
            .with_immutable_features([metadata_2.clone()])
            .replace_immutable_feature(metadata_1.clone());

        let output = builder.clone().finish().unwrap();
        assert_eq!(output.serial_number(), 85);
        assert_eq!(
            output.unlock_conditions().immutable_alias_address(),
            Some(&alias_1)
        );
        assert_eq!(output.features().metadata(), Some(&metadata_2));
        assert_eq!(output.immutable_features().metadata(), Some(&metadata_1));

        builder = builder
            .clear_unlock_conditions()
            .clear_features()
            .clear_immutable_features()
            .replace_unlock_condition(alias_2);
        let output = builder.clone().finish().unwrap();
        assert_eq!(
            output.unlock_conditions().immutable_alias_address(),
            Some(&alias_2)
        );
        assert!(output.features().is_empty());
        assert!(output.immutable_features().is_empty());

        let output = builder
            .with_minimum_storage_deposit(*protocol_parameters.rent_structure())
            .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                rand_alias_address(),
            ))
            .finish_with_params(&protocol_parameters)
            .unwrap();

        assert_eq!(
            output.amount(),
            Output::Foundry(output).rent_cost(protocol_parameters.rent_structure())
        );
    }

    #[test]
    fn pack_unpack() {
        let protocol_parameters = protocol_parameters();
        let output = rand_foundry_output(protocol_parameters.token_supply());
        let bytes = output.pack_to_vec();
        let output_unpacked = FoundryOutput::unpack_verified(bytes, &protocol_parameters).unwrap();
        assert_eq!(output, output_unpacked);
    }

    #[test]
    fn to_from_dto() {
        let protocol_parameters = protocol_parameters();
        let output = rand_foundry_output(protocol_parameters.token_supply());
        let dto = OutputDto::Foundry((&output).into());
        let output_unver = Output::try_from_dto(dto.clone()).unwrap();
        assert_eq!(&output, output_unver.as_foundry());
        let output_ver =
            Output::try_from_dto_with_params(dto, protocol_parameters.clone()).unwrap();
        assert_eq!(&output, output_ver.as_foundry());

        let foundry_id = FoundryId::build(&rand_alias_address(), 0, SimpleTokenScheme::KIND);

        let test_split_dto = |builder: FoundryOutputBuilder| {
            let output_split = FoundryOutput::try_from_dtos(
                (&builder.amount).into(),
                Some(builder.native_tokens.iter().copied().collect()),
                builder.serial_number,
                (&builder.token_scheme).into(),
                builder.unlock_conditions.iter().map(Into::into).collect(),
                Some(builder.features.iter().map(Into::into).collect()),
                Some(builder.immutable_features.iter().map(Into::into).collect()),
                protocol_parameters.clone(),
            )
            .unwrap();
            assert_eq!(
                builder
                    .finish_with_params(protocol_parameters.clone())
                    .unwrap(),
                output_split
            );
        };

        let builder = FoundryOutput::build_with_amount(100, 123, rand_token_scheme())
            .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
            .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                rand_alias_address(),
            ))
            .add_immutable_feature(rand_metadata_feature())
            .with_features(rand_allowed_features(FoundryOutput::ALLOWED_FEATURES));
        test_split_dto(builder);

        let builder = FoundryOutput::build_with_minimum_storage_deposit(
            *protocol_parameters.rent_structure(),
            123,
            rand_token_scheme(),
        )
        .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
        .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
            rand_alias_address(),
        ))
        .add_immutable_feature(rand_metadata_feature())
        .with_features(rand_allowed_features(FoundryOutput::ALLOWED_FEATURES));
        test_split_dto(builder);
    }
}
