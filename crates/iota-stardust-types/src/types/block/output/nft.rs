// Copyright 2021-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use alloc::collections::BTreeSet;

use packable::{
    Packable,
    error::{UnpackError, UnpackErrorExt},
    packer::Packer,
    unpacker::Unpacker,
};

use super::verify_output_amount_packable;
use crate::types::{
    ValidationParams,
    block::{
        Error,
        address::{Address, NftAddress},
        output::{
            ChainId, NativeToken, NativeTokens, NftId, Output, OutputBuilderAmount, OutputId, Rent,
            RentStructure, StateTransitionError, StateTransitionVerifier,
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
pub struct NftOutputBuilder {
    amount: OutputBuilderAmount,
    native_tokens: BTreeSet<NativeToken>,
    nft_id: NftId,
    unlock_conditions: BTreeSet<UnlockCondition>,
    features: BTreeSet<Feature>,
    immutable_features: BTreeSet<Feature>,
}

impl NftOutputBuilder {
    /// Creates an [`NftOutputBuilder`] with a provided amount.
    pub fn new_with_amount(amount: u64, nft_id: NftId) -> Self {
        Self::new(OutputBuilderAmount::Amount(amount), nft_id)
    }

    /// Creates an [`NftOutputBuilder`] with a provided rent structure.
    /// The amount will be set to the minimum storage deposit.
    pub fn new_with_minimum_storage_deposit(rent_structure: RentStructure, nft_id: NftId) -> Self {
        Self::new(
            OutputBuilderAmount::MinimumStorageDeposit(rent_structure),
            nft_id,
        )
    }

    fn new(amount: OutputBuilderAmount, nft_id: NftId) -> Self {
        Self {
            amount,
            native_tokens: BTreeSet::new(),
            nft_id,
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

    /// Sets the NFT ID to the provided value.
    #[inline(always)]
    pub fn with_nft_id(mut self, nft_id: NftId) -> Self {
        self.nft_id = nft_id;
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
    pub fn finish(self) -> Result<NftOutput, Error> {
        let unlock_conditions = UnlockConditions::from_set(self.unlock_conditions)?;

        verify_unlock_conditions(&unlock_conditions, &self.nft_id)?;

        let features = Features::from_set(self.features)?;

        verify_allowed_features(&features, NftOutput::ALLOWED_FEATURES)?;

        let immutable_features = Features::from_set(self.immutable_features)?;

        verify_allowed_features(&immutable_features, NftOutput::ALLOWED_IMMUTABLE_FEATURES)?;

        let mut output = NftOutput {
            amount: 1u64,
            native_tokens: NativeTokens::from_set(self.native_tokens)?,
            nft_id: self.nft_id,
            unlock_conditions,
            features,
            immutable_features,
        };

        output.amount = match self.amount {
            OutputBuilderAmount::Amount(amount) => amount,
            OutputBuilderAmount::MinimumStorageDeposit(rent_structure) => {
                Output::Nft(output.clone()).rent_cost(&rent_structure)
            }
        };

        Ok(output)
    }

    ///
    pub fn finish_with_params<'a>(
        self,
        params: impl Into<ValidationParams<'a>> + Send,
    ) -> Result<NftOutput, Error> {
        let output = self.finish()?;

        if let Some(token_supply) = params.into().token_supply() {
            verify_output_amount(&output.amount, &token_supply)?;
        }

        Ok(output)
    }

    /// Finishes the [`NftOutputBuilder`] into an [`Output`].
    pub fn finish_output<'a>(
        self,
        params: impl Into<ValidationParams<'a>> + Send,
    ) -> Result<Output, Error> {
        Ok(Output::Nft(self.finish_with_params(params)?))
    }
}

impl From<&NftOutput> for NftOutputBuilder {
    fn from(output: &NftOutput) -> Self {
        Self {
            amount: OutputBuilderAmount::Amount(output.amount),
            native_tokens: output.native_tokens.iter().copied().collect(),
            nft_id: output.nft_id,
            unlock_conditions: output.unlock_conditions.iter().cloned().collect(),
            features: output.features.iter().cloned().collect(),
            immutable_features: output.immutable_features.iter().cloned().collect(),
        }
    }
}

/// Describes an NFT output, a globally unique token with metadata attached.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct NftOutput {
    // Amount of IOTA tokens held by the output.
    amount: u64,
    // Native tokens held by the output.
    native_tokens: NativeTokens,
    // Unique identifier of the NFT.
    nft_id: NftId,
    unlock_conditions: UnlockConditions,
    features: Features,
    immutable_features: Features,
}

impl NftOutput {
    /// The [`Output`] kind of an [`NftOutput`].
    pub const KIND: u8 = 6;
    /// The set of allowed [`UnlockCondition`]s for an [`NftOutput`].
    pub const ALLOWED_UNLOCK_CONDITIONS: UnlockConditionFlags = UnlockConditionFlags::ADDRESS
        .union(UnlockConditionFlags::STORAGE_DEPOSIT_RETURN)
        .union(UnlockConditionFlags::TIMELOCK)
        .union(UnlockConditionFlags::EXPIRATION);
    /// The set of allowed [`Feature`]s for an [`NftOutput`].
    pub const ALLOWED_FEATURES: FeatureFlags = FeatureFlags::SENDER
        .union(FeatureFlags::METADATA)
        .union(FeatureFlags::TAG);
    /// The set of allowed immutable [`Feature`]s for an [`NftOutput`].
    pub const ALLOWED_IMMUTABLE_FEATURES: FeatureFlags =
        FeatureFlags::ISSUER.union(FeatureFlags::METADATA);

    /// Creates a new [`NftOutputBuilder`] with a provided amount.
    #[inline(always)]
    pub fn build_with_amount(amount: u64, nft_id: NftId) -> NftOutputBuilder {
        NftOutputBuilder::new_with_amount(amount, nft_id)
    }

    /// Creates a new [`NftOutputBuilder`] with a provided rent structure.
    /// The amount will be set to the minimum storage deposit.
    #[inline(always)]
    pub fn build_with_minimum_storage_deposit(
        rent_structure: RentStructure,
        nft_id: NftId,
    ) -> NftOutputBuilder {
        NftOutputBuilder::new_with_minimum_storage_deposit(rent_structure, nft_id)
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
    pub fn nft_id(&self) -> &NftId {
        &self.nft_id
    }

    /// Returns the nft ID if not null, or creates it from the output ID.
    #[inline(always)]
    pub fn nft_id_non_null(&self, output_id: &OutputId) -> NftId {
        self.nft_id.or_from_output_id(output_id)
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
    pub fn address(&self) -> &Address {
        // An NftOutput must have an AddressUnlockCondition.
        self.unlock_conditions
            .address()
            .map(|unlock_condition| unlock_condition.address())
            .unwrap()
    }

    ///
    #[inline(always)]
    pub fn chain_id(&self) -> ChainId {
        ChainId::Nft(self.nft_id)
    }

    /// Returns the nft address for this output.
    pub fn nft_address(&self, output_id: &OutputId) -> NftAddress {
        NftAddress::new(self.nft_id_non_null(output_id))
    }

    ///
    pub fn unlock(
        &self,
        output_id: &OutputId,
        unlock: &Unlock,
        inputs: &[(&OutputId, &Output)],
        context: &mut ValidationContext<'_>,
    ) -> Result<(), ConflictReason> {
        self.unlock_conditions()
            .locked_address(self.address(), context.milestone_timestamp)
            .unlock(unlock, inputs, context)?;

        let nft_id = if self.nft_id().is_null() {
            NftId::from(output_id)
        } else {
            *self.nft_id()
        };

        context
            .unlocked_addresses
            .insert(Address::from(NftAddress::from(nft_id)));

        Ok(())
    }

    // Transition, just without full ValidationContext
    pub(crate) fn transition_inner(
        current_state: &Self,
        next_state: &Self,
    ) -> Result<(), StateTransitionError> {
        if current_state.immutable_features != next_state.immutable_features {
            return Err(StateTransitionError::MutatedImmutableField);
        }
        Ok(())
    }
}

impl StateTransitionVerifier for NftOutput {
    fn creation(
        next_state: &Self,
        context: &ValidationContext<'_>,
    ) -> Result<(), StateTransitionError> {
        if !next_state.nft_id.is_null() {
            return Err(StateTransitionError::NonZeroCreatedId);
        }

        if let Some(issuer) = next_state.immutable_features().issuer() {
            if !context.unlocked_addresses.contains(issuer.address()) {
                return Err(StateTransitionError::IssuerNotUnlocked);
            }
        }

        Ok(())
    }

    fn transition(
        current_state: &Self,
        next_state: &Self,
        _context: &ValidationContext<'_>,
    ) -> Result<(), StateTransitionError> {
        Self::transition_inner(current_state, next_state)
    }

    fn destruction(
        _current_state: &Self,
        _context: &ValidationContext<'_>,
    ) -> Result<(), StateTransitionError> {
        Ok(())
    }
}

impl Packable for NftOutput {
    type UnpackError = Error;
    type UnpackVisitor = ProtocolParameters;

    fn pack<P: Packer>(&self, packer: &mut P) -> Result<(), P::Error> {
        self.amount.pack(packer)?;
        self.native_tokens.pack(packer)?;
        self.nft_id.pack(packer)?;
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
        let nft_id = NftId::unpack::<_, VERIFY>(unpacker, &()).coerce()?;
        let unlock_conditions = UnlockConditions::unpack::<_, VERIFY>(unpacker, visitor)?;

        if VERIFY {
            verify_unlock_conditions(&unlock_conditions, &nft_id).map_err(UnpackError::Packable)?;
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
            nft_id,
            unlock_conditions,
            features,
            immutable_features,
        })
    }
}

fn verify_unlock_conditions(
    unlock_conditions: &UnlockConditions,
    nft_id: &NftId,
) -> Result<(), Error> {
    if let Some(unlock_condition) = unlock_conditions.address() {
        if let Address::Nft(nft_address) = unlock_condition.address() {
            if !nft_id.is_null() && nft_address.nft_id() == nft_id {
                return Err(Error::SelfDepositNft(*nft_id));
            }
        }
    } else {
        return Err(Error::MissingAddressUnlockCondition);
    }

    verify_allowed_unlock_conditions(unlock_conditions, NftOutput::ALLOWED_UNLOCK_CONDITIONS)
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
                unlock_condition::dto::UnlockConditionDto,
            },
        },
    };

    /// Describes an NFT output, a globally unique token with metadata attached.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct NftOutputDto {
        #[serde(rename = "type")]
        pub kind: u8,
        // Amount of IOTA tokens held by the output.
        pub amount: String,
        // Native tokens held by the output.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub native_tokens: Vec<NativeToken>,
        // Unique identifier of the NFT.
        pub nft_id: NftId,
        pub unlock_conditions: Vec<UnlockConditionDto>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub features: Vec<FeatureDto>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub immutable_features: Vec<FeatureDto>,
    }

    impl From<&NftOutput> for NftOutputDto {
        fn from(value: &NftOutput) -> Self {
            Self {
                kind: NftOutput::KIND,
                amount: value.amount().to_string(),
                native_tokens: value.native_tokens().to_vec(),
                nft_id: *value.nft_id(),
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

    impl TryFromDto for NftOutput {
        type Dto = NftOutputDto;
        type Error = Error;

        fn try_from_dto_with_params_inner(
            dto: Self::Dto,
            params: ValidationParams<'_>,
        ) -> Result<Self, Self::Error> {
            let mut builder = NftOutputBuilder::new_with_amount(
                dto.amount
                    .parse::<u64>()
                    .map_err(|_| Error::InvalidField("amount"))?,
                dto.nft_id,
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

    impl NftOutput {
        pub fn try_from_dtos<'a>(
            amount: OutputBuilderAmountDto,
            native_tokens: Option<Vec<NativeToken>>,
            nft_id: &NftId,
            unlock_conditions: Vec<UnlockConditionDto>,
            features: Option<Vec<FeatureDto>>,
            immutable_features: Option<Vec<FeatureDto>>,
            params: impl Into<ValidationParams<'a>> + Send,
        ) -> Result<Self, Error> {
            let params = params.into();
            let mut builder = match amount {
                OutputBuilderAmountDto::Amount(amount) => NftOutputBuilder::new_with_amount(
                    amount.parse().map_err(|_| Error::InvalidField("amount"))?,
                    *nft_id,
                ),
                OutputBuilderAmountDto::MinimumStorageDeposit(rent_structure) => {
                    NftOutputBuilder::new_with_minimum_storage_deposit(rent_structure, *nft_id)
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
                FoundryId, SimpleTokenScheme, TokenId,
                dto::{OutputBuilderAmountDto, OutputDto},
            },
            protocol::protocol_parameters,
            rand::{
                address::rand_alias_address,
                output::{
                    feature::{rand_allowed_features, rand_issuer_feature, rand_sender_feature},
                    rand_nft_output,
                    unlock_condition::rand_address_unlock_condition,
                },
            },
        },
    };

    #[test]
    fn builder() {
        let protocol_parameters = protocol_parameters();
        let foundry_id = FoundryId::build(&rand_alias_address(), 0, SimpleTokenScheme::KIND);
        let address_1 = rand_address_unlock_condition();
        let address_2 = rand_address_unlock_condition();
        let sender_1 = rand_sender_feature();
        let sender_2 = rand_sender_feature();
        let issuer_1 = rand_issuer_feature();
        let issuer_2 = rand_issuer_feature();

        let mut builder = NftOutput::build_with_amount(0, NftId::null())
            .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
            .add_unlock_condition(address_1)
            .add_feature(sender_1)
            .replace_feature(sender_2)
            .replace_immutable_feature(issuer_1)
            .add_immutable_feature(issuer_2);

        let output = builder.clone().finish().unwrap();
        assert_eq!(output.unlock_conditions().address(), Some(&address_1));
        assert_eq!(output.features().sender(), Some(&sender_2));
        assert_eq!(output.immutable_features().issuer(), Some(&issuer_1));

        builder = builder
            .clear_unlock_conditions()
            .clear_features()
            .clear_immutable_features()
            .replace_unlock_condition(address_2);
        let output = builder.clone().finish().unwrap();
        assert_eq!(output.unlock_conditions().address(), Some(&address_2));
        assert!(output.features().is_empty());
        assert!(output.immutable_features().is_empty());

        let output = builder
            .with_minimum_storage_deposit(*protocol_parameters.rent_structure())
            .add_unlock_condition(rand_address_unlock_condition())
            .finish_with_params(protocol_parameters.token_supply())
            .unwrap();

        assert_eq!(
            output.amount(),
            Output::Nft(output).rent_cost(protocol_parameters.rent_structure())
        );
    }

    #[test]
    fn pack_unpack() {
        let protocol_parameters = protocol_parameters();
        let output = rand_nft_output(protocol_parameters.token_supply());
        let bytes = output.pack_to_vec();
        let output_unpacked = NftOutput::unpack_verified(bytes, &protocol_parameters).unwrap();
        assert_eq!(output, output_unpacked);
    }

    #[test]
    fn to_from_dto() {
        let protocol_parameters = protocol_parameters();
        let output = rand_nft_output(protocol_parameters.token_supply());
        let dto = OutputDto::Nft((&output).into());
        let output_unver = Output::try_from_dto(dto.clone()).unwrap();
        assert_eq!(&output, output_unver.as_nft());
        let output_ver = Output::try_from_dto_with_params(dto, &protocol_parameters).unwrap();
        assert_eq!(&output, output_ver.as_nft());

        let foundry_id = FoundryId::build(&rand_alias_address(), 0, SimpleTokenScheme::KIND);

        let output_split = NftOutput::try_from_dtos(
            OutputBuilderAmountDto::Amount(output.amount().to_string()),
            Some(output.native_tokens().to_vec()),
            output.nft_id(),
            output.unlock_conditions().iter().map(Into::into).collect(),
            Some(output.features().iter().map(Into::into).collect()),
            Some(output.immutable_features().iter().map(Into::into).collect()),
            &protocol_parameters,
        )
        .unwrap();
        assert_eq!(output, output_split);

        let test_split_dto = |builder: NftOutputBuilder| {
            let output_split = NftOutput::try_from_dtos(
                (&builder.amount).into(),
                Some(builder.native_tokens.iter().copied().collect()),
                &builder.nft_id,
                builder.unlock_conditions.iter().map(Into::into).collect(),
                Some(builder.features.iter().map(Into::into).collect()),
                Some(builder.immutable_features.iter().map(Into::into).collect()),
                &protocol_parameters,
            )
            .unwrap();
            assert_eq!(
                builder.finish_with_params(&protocol_parameters).unwrap(),
                output_split
            );
        };

        let builder = NftOutput::build_with_amount(100, NftId::null())
            .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
            .add_unlock_condition(rand_address_unlock_condition())
            .with_features(rand_allowed_features(NftOutput::ALLOWED_FEATURES))
            .with_immutable_features(rand_allowed_features(NftOutput::ALLOWED_IMMUTABLE_FEATURES));
        test_split_dto(builder);

        let builder = NftOutput::build_with_minimum_storage_deposit(
            *protocol_parameters.rent_structure(),
            NftId::null(),
        )
        .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
        .add_unlock_condition(rand_address_unlock_condition())
        .with_features(rand_allowed_features(NftOutput::ALLOWED_FEATURES))
        .with_immutable_features(rand_allowed_features(NftOutput::ALLOWED_IMMUTABLE_FEATURES));
        test_split_dto(builder);
    }
}
