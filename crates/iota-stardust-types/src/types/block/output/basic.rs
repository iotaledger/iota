// Copyright 2021-2022 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use alloc::collections::BTreeSet;

use packable::Packable;

use super::verify_output_amount_packable;
use crate::types::{
    ValidationParams,
    block::{
        Error,
        address::Address,
        output::{
            NativeToken, NativeTokens, Output, OutputBuilderAmount, OutputId, Rent, RentStructure,
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
pub struct BasicOutputBuilder {
    amount: OutputBuilderAmount,
    native_tokens: BTreeSet<NativeToken>,
    unlock_conditions: BTreeSet<UnlockCondition>,
    features: BTreeSet<Feature>,
}

impl BasicOutputBuilder {
    /// Creates a [`BasicOutputBuilder`] with a provided amount.
    #[inline(always)]
    pub fn new_with_amount(amount: u64) -> Self {
        Self::new(OutputBuilderAmount::Amount(amount))
    }

    /// Creates an [`BasicOutputBuilder`] with a provided rent structure.
    /// The amount will be set to the minimum storage deposit.
    #[inline(always)]
    pub fn new_with_minimum_storage_deposit(rent_structure: RentStructure) -> Self {
        Self::new(OutputBuilderAmount::MinimumStorageDeposit(rent_structure))
    }

    fn new(amount: OutputBuilderAmount) -> Self {
        Self {
            amount,
            native_tokens: BTreeSet::new(),
            unlock_conditions: BTreeSet::new(),
            features: BTreeSet::new(),
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

    ///
    pub fn finish(self) -> Result<BasicOutput, Error> {
        let unlock_conditions = UnlockConditions::from_set(self.unlock_conditions)?;

        verify_unlock_conditions::<true>(&unlock_conditions)?;

        let features = Features::from_set(self.features)?;

        verify_features::<true>(&features)?;

        let mut output = BasicOutput {
            amount: 1u64,
            native_tokens: NativeTokens::from_set(self.native_tokens)?,
            unlock_conditions,
            features,
        };

        output.amount = match self.amount {
            OutputBuilderAmount::Amount(amount) => amount,
            OutputBuilderAmount::MinimumStorageDeposit(rent_structure) => {
                Output::Basic(output.clone()).rent_cost(&rent_structure)
            }
        };

        Ok(output)
    }

    ///
    pub fn finish_with_params<'a>(
        self,
        params: impl Into<ValidationParams<'a>> + Send,
    ) -> Result<BasicOutput, Error> {
        let output = self.finish()?;

        if let Some(token_supply) = params.into().token_supply() {
            verify_output_amount(&output.amount, &token_supply)?;
        }

        Ok(output)
    }

    /// Finishes the [`BasicOutputBuilder`] into an [`Output`].
    pub fn finish_output<'a>(
        self,
        params: impl Into<ValidationParams<'a>> + Send,
    ) -> Result<Output, Error> {
        Ok(Output::Basic(self.finish_with_params(params)?))
    }
}

impl From<&BasicOutput> for BasicOutputBuilder {
    fn from(output: &BasicOutput) -> Self {
        Self {
            amount: OutputBuilderAmount::Amount(output.amount),
            native_tokens: output.native_tokens.iter().copied().collect(),
            unlock_conditions: output.unlock_conditions.iter().cloned().collect(),
            features: output.features.iter().cloned().collect(),
        }
    }
}

/// Describes a basic output with optional features.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Packable)]
#[packable(unpack_error = Error)]
#[packable(unpack_visitor = ProtocolParameters)]
pub struct BasicOutput {
    // Amount of IOTA tokens held by the output.
    #[packable(verify_with = verify_output_amount_packable)]
    amount: u64,
    // Native tokens held by the output.
    native_tokens: NativeTokens,
    #[packable(verify_with = verify_unlock_conditions_packable)]
    unlock_conditions: UnlockConditions,
    #[packable(verify_with = verify_features_packable)]
    features: Features,
}

impl BasicOutput {
    /// The [`Output`] kind of an [`BasicOutput`].
    pub const KIND: u8 = 3;

    /// The set of allowed [`UnlockCondition`]s for an [`BasicOutput`].
    const ALLOWED_UNLOCK_CONDITIONS: UnlockConditionFlags = UnlockConditionFlags::ADDRESS
        .union(UnlockConditionFlags::STORAGE_DEPOSIT_RETURN)
        .union(UnlockConditionFlags::TIMELOCK)
        .union(UnlockConditionFlags::EXPIRATION);
    /// The set of allowed [`Feature`]s for an [`BasicOutput`].
    pub const ALLOWED_FEATURES: FeatureFlags = FeatureFlags::SENDER
        .union(FeatureFlags::METADATA)
        .union(FeatureFlags::TAG);

    /// Creates a new [`BasicOutputBuilder`] with a provided amount.
    #[inline(always)]
    pub fn build_with_amount(amount: u64) -> BasicOutputBuilder {
        BasicOutputBuilder::new_with_amount(amount)
    }

    /// Creates a new [`BasicOutputBuilder`] with a provided rent structure.
    /// The amount will be set to the minimum storage deposit.
    #[inline(always)]
    pub fn build_with_minimum_storage_deposit(rent_structure: RentStructure) -> BasicOutputBuilder {
        BasicOutputBuilder::new_with_minimum_storage_deposit(rent_structure)
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
    pub fn address(&self) -> &Address {
        // An BasicOutput must have an AddressUnlockCondition.
        self.unlock_conditions
            .address()
            .map(|unlock_condition| unlock_condition.address())
            .unwrap()
    }

    ///
    pub fn unlock(
        &self,
        _output_id: &OutputId,
        unlock: &Unlock,
        inputs: &[(&OutputId, &Output)],
        context: &mut ValidationContext<'_>,
    ) -> Result<(), ConflictReason> {
        self.unlock_conditions()
            .locked_address(self.address(), context.milestone_timestamp)
            .unlock(unlock, inputs, context)
    }

    /// Returns the address of the unlock conditions if the output is a simple
    /// deposit. Simple deposit outputs are basic outputs with only an
    /// address unlock condition, no native tokens and no features. They are
    /// used to return storage deposits.
    pub fn simple_deposit_address(&self) -> Option<&Address> {
        if let [UnlockCondition::Address(address)] = self.unlock_conditions().as_ref() {
            if self.native_tokens.is_empty() && self.features.is_empty() {
                return Some(address.address());
            }
        }

        None
    }
}

fn verify_unlock_conditions<const VERIFY: bool>(
    unlock_conditions: &UnlockConditions,
) -> Result<(), Error> {
    if VERIFY {
        if unlock_conditions.address().is_none() {
            Err(Error::MissingAddressUnlockCondition)
        } else {
            verify_allowed_unlock_conditions(
                unlock_conditions,
                BasicOutput::ALLOWED_UNLOCK_CONDITIONS,
            )
        }
    } else {
        Ok(())
    }
}

fn verify_unlock_conditions_packable<const VERIFY: bool>(
    unlock_conditions: &UnlockConditions,
    _: &ProtocolParameters,
) -> Result<(), Error> {
    verify_unlock_conditions::<VERIFY>(unlock_conditions)
}

fn verify_features<const VERIFY: bool>(blocks: &Features) -> Result<(), Error> {
    if VERIFY {
        verify_allowed_features(blocks, BasicOutput::ALLOWED_FEATURES)
    } else {
        Ok(())
    }
}

fn verify_features_packable<const VERIFY: bool>(
    blocks: &Features,
    _: &ProtocolParameters,
) -> Result<(), Error> {
    verify_features::<VERIFY>(blocks)
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

    /// Describes a basic output.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct BasicOutputDto {
        #[serde(rename = "type")]
        pub kind: u8,
        // Amount of IOTA tokens held by the output.
        pub amount: String,
        // Native tokens held by the output.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub native_tokens: Vec<NativeToken>,
        pub unlock_conditions: Vec<UnlockConditionDto>,
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        pub features: Vec<FeatureDto>,
    }

    impl From<&BasicOutput> for BasicOutputDto {
        fn from(value: &BasicOutput) -> Self {
            Self {
                kind: BasicOutput::KIND,
                amount: value.amount().to_string(),
                native_tokens: value.native_tokens().to_vec(),
                unlock_conditions: value
                    .unlock_conditions()
                    .iter()
                    .map(Into::into)
                    .collect::<_>(),
                features: value.features().iter().map(Into::into).collect::<_>(),
            }
        }
    }

    impl TryFromDto for BasicOutput {
        type Dto = BasicOutputDto;
        type Error = Error;

        fn try_from_dto_with_params_inner(
            dto: Self::Dto,
            params: ValidationParams<'_>,
        ) -> Result<Self, Self::Error> {
            let mut builder = BasicOutputBuilder::new_with_amount(
                dto.amount
                    .parse()
                    .map_err(|_| Error::InvalidField("amount"))?,
            );

            builder = builder.with_native_tokens(dto.native_tokens);

            for b in dto.features {
                builder = builder.add_feature(Feature::try_from(b)?);
            }

            for u in dto.unlock_conditions {
                builder = builder
                    .add_unlock_condition(UnlockCondition::try_from_dto_with_params(u, &params)?);
            }

            builder.finish_with_params(params)
        }
    }

    impl BasicOutput {
        pub fn try_from_dtos<'a>(
            amount: OutputBuilderAmountDto,
            native_tokens: Option<Vec<NativeToken>>,
            unlock_conditions: Vec<UnlockConditionDto>,
            features: Option<Vec<FeatureDto>>,
            params: impl Into<ValidationParams<'a>> + Send,
        ) -> Result<Self, Error> {
            let params = params.into();
            let mut builder = match amount {
                OutputBuilderAmountDto::Amount(amount) => BasicOutputBuilder::new_with_amount(
                    amount.parse().map_err(|_| Error::InvalidField("amount"))?,
                ),
                OutputBuilderAmountDto::MinimumStorageDeposit(rent_structure) => {
                    BasicOutputBuilder::new_with_minimum_storage_deposit(rent_structure)
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
                    feature::{rand_allowed_features, rand_metadata_feature, rand_sender_feature},
                    rand_basic_output,
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

        let mut builder = BasicOutput::build_with_amount(0)
            .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
            .add_unlock_condition(address_1)
            .add_feature(sender_1)
            .replace_feature(sender_2);

        let output = builder.clone().finish().unwrap();
        assert_eq!(output.unlock_conditions().address(), Some(&address_1));
        assert_eq!(output.features().sender(), Some(&sender_2));

        builder = builder
            .clear_unlock_conditions()
            .clear_features()
            .replace_unlock_condition(address_2);
        let output = builder.clone().finish().unwrap();
        assert_eq!(output.unlock_conditions().address(), Some(&address_2));
        assert!(output.features().is_empty());

        let metadata = rand_metadata_feature();

        let output = builder
            .with_minimum_storage_deposit(*protocol_parameters.rent_structure())
            .add_unlock_condition(rand_address_unlock_condition())
            .with_features([Feature::from(metadata.clone()), sender_1.into()])
            .finish_with_params(
                ValidationParams::default().with_protocol_parameters(protocol_parameters.clone()),
            )
            .unwrap();

        assert_eq!(
            output.amount(),
            Output::Basic(output.clone()).rent_cost(protocol_parameters.rent_structure())
        );
        assert_eq!(output.features().metadata(), Some(&metadata));
        assert_eq!(output.features().sender(), Some(&sender_1));
    }

    #[test]
    fn pack_unpack() {
        let protocol_parameters = protocol_parameters();
        let output = rand_basic_output(protocol_parameters.token_supply());
        let bytes = output.pack_to_vec();
        let output_unpacked = BasicOutput::unpack_verified(bytes, &protocol_parameters).unwrap();
        assert_eq!(output, output_unpacked);
    }

    #[test]
    fn to_from_dto() {
        let protocol_parameters = protocol_parameters();
        let output = rand_basic_output(protocol_parameters.token_supply());
        let dto = OutputDto::Basic((&output).into());
        let output_unver = Output::try_from_dto(dto.clone()).unwrap();
        assert_eq!(&output, output_unver.as_basic());
        let output_ver = Output::try_from_dto_with_params(dto, &protocol_parameters).unwrap();
        assert_eq!(&output, output_ver.as_basic());

        let output_split = BasicOutput::try_from_dtos(
            OutputBuilderAmountDto::Amount(output.amount().to_string()),
            Some(output.native_tokens().to_vec()),
            output.unlock_conditions().iter().map(Into::into).collect(),
            Some(output.features().iter().map(Into::into).collect()),
            protocol_parameters.token_supply(),
        )
        .unwrap();
        assert_eq!(output, output_split);

        let foundry_id = FoundryId::build(&rand_alias_address(), 0, SimpleTokenScheme::KIND);
        let address = rand_address_unlock_condition();

        let test_split_dto = |builder: BasicOutputBuilder| {
            let output_split = BasicOutput::try_from_dtos(
                (&builder.amount).into(),
                Some(builder.native_tokens.iter().copied().collect()),
                builder.unlock_conditions.iter().map(Into::into).collect(),
                Some(builder.features.iter().map(Into::into).collect()),
                protocol_parameters.token_supply(),
            )
            .unwrap();
            assert_eq!(
                builder
                    .finish_with_params(protocol_parameters.token_supply())
                    .unwrap(),
                output_split
            );
        };

        let builder = BasicOutput::build_with_amount(100)
            .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
            .add_unlock_condition(address)
            .with_features(rand_allowed_features(BasicOutput::ALLOWED_FEATURES));
        test_split_dto(builder);

        let builder =
            BasicOutput::build_with_minimum_storage_deposit(*protocol_parameters.rent_structure())
                .add_native_token(NativeToken::new(TokenId::from(foundry_id), 1000).unwrap())
                .add_unlock_condition(address)
                .with_features(rand_allowed_features(BasicOutput::ALLOWED_FEATURES));
        test_split_dto(builder);
    }
}
