// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use alloc::vec::Vec;

use hashbrown::HashSet;
use packable::{Packable, bounded::BoundedU16, prefix::BoxedSlicePrefix};

use crate::types::{
    ValidationParams,
    block::{
        Error,
        input::{INPUT_COUNT_RANGE, Input},
        output::{InputsCommitment, NativeTokens, OUTPUT_COUNT_RANGE, Output, TokenId},
        payload::{OptionalPayload, Payload},
        protocol::ProtocolParameters,
    },
};

/// A builder to build a [`RegularTransactionEssence`].
#[derive(Debug, Clone)]
#[must_use]
pub struct RegularTransactionEssenceBuilder {
    network_id: u64,
    inputs: Vec<Input>,
    inputs_commitment: InputsCommitment,
    outputs: Vec<Output>,
    payload: OptionalPayload,
}

impl RegularTransactionEssenceBuilder {
    /// Creates a new [`RegularTransactionEssenceBuilder`].
    pub fn new(network_id: u64, inputs_commitment: InputsCommitment) -> Self {
        Self {
            network_id,
            inputs: Vec::new(),
            inputs_commitment,
            outputs: Vec::new(),
            payload: OptionalPayload::default(),
        }
    }

    /// Adds inputs to a [`RegularTransactionEssenceBuilder`].
    pub fn with_inputs(mut self, inputs: impl Into<Vec<Input>>) -> Self {
        self.inputs = inputs.into();
        self
    }

    /// Add an input to a [`RegularTransactionEssenceBuilder`].
    pub fn add_input(mut self, input: Input) -> Self {
        self.inputs.push(input);
        self
    }

    /// Add outputs to a [`RegularTransactionEssenceBuilder`].
    pub fn with_outputs(mut self, outputs: impl Into<Vec<Output>>) -> Self {
        self.outputs = outputs.into();
        self
    }

    /// Add an output to a [`RegularTransactionEssenceBuilder`].
    pub fn add_output(mut self, output: Output) -> Self {
        self.outputs.push(output);
        self
    }

    /// Add a payload to a [`RegularTransactionEssenceBuilder`].
    pub fn with_payload(mut self, payload: impl Into<OptionalPayload>) -> Self {
        self.payload = payload.into();
        self
    }

    /// Finishes a [`RegularTransactionEssenceBuilder`] into a
    /// [`RegularTransactionEssence`].
    pub fn finish_with_params<'a>(
        self,
        params: impl Into<ValidationParams<'a>> + Send,
    ) -> Result<RegularTransactionEssence, Error> {
        let params = params.into();
        if let Some(protocol_parameters) = params.protocol_parameters() {
            if self.network_id != protocol_parameters.network_id() {
                return Err(Error::NetworkIdMismatch {
                    expected: protocol_parameters.network_id(),
                    actual: self.network_id,
                });
            }
        }

        let inputs: BoxedSlicePrefix<Input, InputCount> = self
            .inputs
            .into_boxed_slice()
            .try_into()
            .map_err(Error::InvalidInputCount)?;

        verify_inputs::<true>(&inputs)?;

        let outputs: BoxedSlicePrefix<Output, OutputCount> = self
            .outputs
            .into_boxed_slice()
            .try_into()
            .map_err(Error::InvalidOutputCount)?;

        if let Some(protocol_parameters) = params.protocol_parameters() {
            verify_outputs::<true>(&outputs, protocol_parameters)?;
        }

        verify_payload::<true>(&self.payload)?;

        Ok(RegularTransactionEssence {
            network_id: self.network_id,
            inputs,
            inputs_commitment: self.inputs_commitment,
            outputs,
            payload: self.payload,
        })
    }

    ///
    pub fn finish(self) -> Result<RegularTransactionEssence, Error> {
        self.finish_with_params(ValidationParams::default())
    }
}

pub(crate) type InputCount =
    BoundedU16<{ *INPUT_COUNT_RANGE.start() }, { *INPUT_COUNT_RANGE.end() }>;
pub(crate) type OutputCount =
    BoundedU16<{ *OUTPUT_COUNT_RANGE.start() }, { *OUTPUT_COUNT_RANGE.end() }>;

/// A transaction regular essence consuming inputs, creating outputs and
/// carrying an optional payload.
#[derive(Clone, Debug, Eq, PartialEq, Packable)]
#[packable(unpack_error = Error)]
#[packable(unpack_visitor = ProtocolParameters)]
pub struct RegularTransactionEssence {
    /// The unique value denoting whether the block was meant for mainnet,
    /// testnet, or a private network.
    #[packable(verify_with = verify_network_id)]
    network_id: u64,
    #[packable(verify_with = verify_inputs_packable)]
    #[packable(unpack_error_with = |e| e.unwrap_item_err_or_else(|p| Error::InvalidInputCount(p.into())))]
    inputs: BoxedSlicePrefix<Input, InputCount>,
    /// BLAKE2b-256 hash of the serialized outputs referenced in inputs by their
    /// OutputId.
    inputs_commitment: InputsCommitment,
    #[packable(verify_with = verify_outputs)]
    #[packable(unpack_error_with = |e| e.unwrap_item_err_or_else(|p| Error::InvalidOutputCount(p.into())))]
    outputs: BoxedSlicePrefix<Output, OutputCount>,
    #[packable(verify_with = verify_payload_packable)]
    payload: OptionalPayload,
}

impl RegularTransactionEssence {
    /// The essence kind of a [`RegularTransactionEssence`].
    pub const KIND: u8 = 1;

    /// Creates a new [`RegularTransactionEssenceBuilder`] to build a
    /// [`RegularTransactionEssence`].
    pub fn builder(
        network_id: u64,
        inputs_commitment: InputsCommitment,
    ) -> RegularTransactionEssenceBuilder {
        RegularTransactionEssenceBuilder::new(network_id, inputs_commitment)
    }

    /// Returns the network ID of a [`RegularTransactionEssence`].
    pub fn network_id(&self) -> u64 {
        self.network_id
    }

    /// Returns the inputs of a [`RegularTransactionEssence`].
    pub fn inputs(&self) -> &[Input] {
        &self.inputs
    }

    /// Returns the inputs commitment of a [`RegularTransactionEssence`].
    pub fn inputs_commitment(&self) -> &InputsCommitment {
        &self.inputs_commitment
    }

    /// Returns the outputs of a [`RegularTransactionEssence`].
    pub fn outputs(&self) -> &[Output] {
        &self.outputs
    }

    /// Returns the optional payload of a [`RegularTransactionEssence`].
    pub fn payload(&self) -> Option<&Payload> {
        self.payload.as_ref()
    }
}

fn verify_network_id<const VERIFY: bool>(
    network_id: &u64,
    visitor: &ProtocolParameters,
) -> Result<(), Error> {
    if VERIFY {
        let expected = visitor.network_id();

        if *network_id != expected {
            return Err(Error::NetworkIdMismatch {
                expected,
                actual: *network_id,
            });
        }
    }

    Ok(())
}

fn verify_inputs<const VERIFY: bool>(inputs: &[Input]) -> Result<(), Error> {
    if VERIFY {
        let mut seen_utxos = HashSet::new();

        for input in inputs.iter() {
            match input {
                Input::Utxo(utxo) => {
                    if !seen_utxos.insert(utxo) {
                        return Err(Error::DuplicateUtxo(*utxo));
                    }
                }
                _ => return Err(Error::InvalidInputKind(input.kind())),
            }
        }
    }

    Ok(())
}

fn verify_inputs_packable<const VERIFY: bool>(
    inputs: &[Input],
    _visitor: &ProtocolParameters,
) -> Result<(), Error> {
    verify_inputs::<VERIFY>(inputs)
}

fn verify_outputs<const VERIFY: bool>(
    outputs: &[Output],
    visitor: &ProtocolParameters,
) -> Result<(), Error> {
    if VERIFY {
        let mut amount_sum: u64 = 0;
        let mut total_native_tokens = HashSet::<TokenId>::new();
        let mut chain_ids = HashSet::new();

        for output in outputs.iter() {
            let (amount, native_tokens, chain_id) = match output {
                Output::Basic(output) => (output.amount(), output.native_tokens(), None),
                Output::Alias(output) => (
                    output.amount(),
                    output.native_tokens(),
                    Some(output.chain_id()),
                ),
                Output::Foundry(output) => (
                    output.amount(),
                    output.native_tokens(),
                    Some(output.chain_id()),
                ),
                Output::Nft(output) => (
                    output.amount(),
                    output.native_tokens(),
                    Some(output.chain_id()),
                ),
                _ => return Err(Error::InvalidOutputKind(output.kind())),
            };

            amount_sum =
                amount_sum
                    .checked_add(amount)
                    .ok_or(Error::InvalidTransactionAmountSum(
                        amount_sum as u128 + amount as u128,
                    ))?;

            // Accumulated output balance must not exceed the total supply of tokens.
            if amount_sum > visitor.token_supply() {
                return Err(Error::InvalidTransactionAmountSum(amount_sum as u128));
            }

            total_native_tokens.extend(native_tokens.iter().map(|n| n.token_id()));

            if total_native_tokens.len() > NativeTokens::COUNT_MAX.into() {
                return Err(Error::InvalidTransactionNativeTokensCount(
                    total_native_tokens.len() as u16,
                ));
            }

            if let Some(chain_id) = chain_id {
                if !chain_id.is_null() && !chain_ids.insert(chain_id) {
                    return Err(Error::DuplicateOutputChain(chain_id));
                }
            }

            output.verify_storage_deposit(*visitor.rent_structure(), visitor.token_supply())?;
        }
    }

    Ok(())
}

fn verify_payload<const VERIFY: bool>(payload: &OptionalPayload) -> Result<(), Error> {
    if VERIFY {
        match &payload.0 {
            Some(Payload::TaggedData(_)) | None => Ok(()),
            Some(payload) => Err(Error::InvalidPayloadKind(payload.kind())),
        }
    } else {
        Ok(())
    }
}

fn verify_payload_packable<const VERIFY: bool>(
    payload: &OptionalPayload,
    _visitor: &ProtocolParameters,
) -> Result<(), Error> {
    verify_payload::<VERIFY>(payload)
}

#[cfg(feature = "serde")]
pub(crate) mod dto {
    use alloc::{
        boxed::Box,
        string::{String, ToString},
    };
    use core::str::FromStr;

    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::types::{
        TryFromDto,
        block::{Error, input::dto::InputDto, output::dto::OutputDto, payload::dto::PayloadDto},
    };

    /// Describes the essence data making up a transaction by defining its
    /// inputs and outputs and an optional payload.
    #[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RegularTransactionEssenceDto {
        #[serde(rename = "type")]
        pub kind: u8,
        pub network_id: String,
        pub inputs: Vec<InputDto>,
        pub inputs_commitment: String,
        pub outputs: Vec<OutputDto>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub payload: Option<PayloadDto>,
    }

    impl From<&RegularTransactionEssence> for RegularTransactionEssenceDto {
        fn from(value: &RegularTransactionEssence) -> Self {
            Self {
                kind: RegularTransactionEssence::KIND,
                network_id: value.network_id().to_string(),
                inputs: value.inputs().iter().map(Into::into).collect::<Vec<_>>(),
                inputs_commitment: value.inputs_commitment().to_string(),
                outputs: value.outputs().iter().map(Into::into).collect::<Vec<_>>(),
                payload: match value.payload() {
                    Some(Payload::TaggedData(i)) => {
                        Some(PayloadDto::TaggedData(Box::new(i.as_ref().into())))
                    }
                    Some(_) => unimplemented!(),
                    None => None,
                },
            }
        }
    }

    impl TryFromDto for RegularTransactionEssence {
        type Dto = RegularTransactionEssenceDto;
        type Error = Error;

        fn try_from_dto_with_params_inner(
            dto: Self::Dto,
            params: ValidationParams<'_>,
        ) -> Result<Self, Self::Error> {
            let outputs = dto
                .outputs
                .into_iter()
                .map(|o| Output::try_from_dto_with_params(o, &params))
                .collect::<Result<Vec<Output>, Error>>()?;

            let network_id = dto
                .network_id
                .parse::<u64>()
                .map_err(|_| Error::InvalidField("network_id"))?;
            let inputs = dto
                .inputs
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<Input>, Error>>()?;

            let mut builder = Self::builder(
                network_id,
                InputsCommitment::from_str(&dto.inputs_commitment)?,
            )
            .with_inputs(inputs)
            .with_outputs(outputs);

            builder = if let Some(p) = dto.payload {
                if let PayloadDto::TaggedData(i) = p {
                    builder.with_payload(Payload::TaggedData(Box::new((*i).try_into()?)))
                } else {
                    return Err(Error::InvalidField("payload"));
                }
            } else {
                builder
            };

            builder.finish_with_params(params)
        }
    }
}
