// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Stardust alias, basic, nft, foundry outputs.
//! Multi mnemonics, multi accounts, multi addresses.

use iota_sdk::{
    client::secret::{GenerateAddressOptions, SecretManage, mnemonic::MnemonicSecretManager},
    types::block::{
        address::{AliasAddress, Ed25519Address},
        output::{
            AliasId, AliasOutputBuilder, BasicOutputBuilder, FoundryId, FoundryOutputBuilder,
            NativeToken, NftId, NftOutputBuilder, Output, SimpleTokenScheme, TokenId, TokenScheme,
            UnlockCondition,
            feature::{
                Feature, Irc27Metadata, Irc30Metadata, IssuerFeature, MetadataFeature,
                SenderFeature, TagFeature,
            },
            unlock_condition::{
                AddressUnlockCondition, ExpirationUnlockCondition, GovernorAddressUnlockCondition,
                ImmutableAliasAddressUnlockCondition, StateControllerAddressUnlockCondition,
                StorageDepositReturnUnlockCondition, TimelockUnlockCondition,
            },
        },
    },
};
use rand::{Rng, rngs::StdRng};

use crate::stardust::{
    test_outputs::{MERGE_MILESTONE_INDEX, MERGE_TIMESTAMP_SECS, new_vested_output},
    types::{output_header::OutputHeader, output_index::random_output_index_with_rng},
};

const OUTPUT_IOTA_AMOUNT: u64 = 1_000_000;
const STORAGE_DEPOSIT_AMOUNT: u64 = 500_000;

struct StardustWallet {
    mnemonic: &'static str,
    // bip path values for account, internal, address
    addresses: &'static [[u32; 3]],
}

const STARDUST_MIX: &[StardustWallet] = &[
    // First public address only
    StardustWallet {
        mnemonic: "chest inquiry stick anger scheme tail void cup toe game copy jump law bone risk pull crowd dry raw baby want tip oak dice",
        addresses: &[[0, 0, 0]],
    },
    // Multiple public addresses
    StardustWallet {
        mnemonic: "okay pottery arch air egg very cave cash poem gown sorry mind poem crack dawn wet car pink extra crane hen bar boring salt",
        addresses: &[[0, 0, 0], [0, 0, 1], [0, 0, 2], [0, 0, 5]],
    },
    // Multiple internal addresses
    StardustWallet {
        mnemonic: "face tag all fade win east asset taxi holiday need slow fold play pull away earn bus room run one kidney mail design space",
        addresses: &[[0, 1, 1], [0, 1, 2], [0, 1, 5]],
    },
    // Multiple public and internal addresses
    StardustWallet {
        mnemonic: "rain flip mad lamp owner siren tower buddy wolf shy tray exit glad come dry tent they pond wrist web cliff mixed seek drum",
        addresses: &[
            // public
            [0, 0, 0],
            [0, 0, 1],
            [0, 0, 2],
            [0, 0, 5],
            // internal
            [0, 1, 1],
            [0, 1, 2],
            [0, 1, 5],
        ],
    },
    // Multiple accounts multiple public and internal addresses
    StardustWallet {
        mnemonic: "oak eye use bus high enact city desk gaze sure radio text ice food give foil raw dove attitude van clap tenant human other",
        addresses: &[
            // account 2
            // public
            [2, 0, 0],
            [2, 0, 1],
            [2, 0, 2],
            [2, 0, 5],
            // internal
            [2, 1, 1],
            [2, 1, 2],
            [2, 1, 5],
            // account 7
            // public
            [7, 0, 0],
            [7, 0, 1],
            [7, 0, 2],
            [7, 0, 5],
            // internal
            [7, 1, 1],
            [7, 1, 2],
            [7, 1, 5],
            // account 12
            // public
            [12, 0, 0],
            [12, 0, 1],
            [12, 0, 2],
            [12, 0, 5],
            // internal
            [12, 1, 1],
            [12, 1, 2],
            [12, 1, 5],
        ],
    },
    // Everything crazy
    StardustWallet {
        mnemonic: "crazy drum raw dirt tooth where fee base warm beach trim rule sign silk fee fee dad large creek venue coin steel hub scale",
        addresses: &[
            // account 0
            // public
            [0, 0, 5],
            [0, 0, 10],
            [0, 0, 20],
            [0, 0, 50],
            // internal
            [0, 1, 10],
            [0, 1, 20],
            [0, 1, 50],
            // account 2
            // public
            [2, 0, 0],
            [2, 0, 10],
            [2, 0, 20],
            [2, 0, 50],
            // internal
            [2, 1, 10],
            [2, 1, 20],
            [2, 1, 50],
            // account 10
            // public
            [10, 0, 0],
            [10, 0, 10],
            [10, 0, 20],
            [10, 0, 50],
            // internal
            [10, 1, 10],
            [10, 1, 20],
            [10, 1, 50],
            // account 50
            // public
            [50, 0, 0],
            [50, 0, 10],
            [50, 0, 20],
            [50, 0, 50],
            // internal
            [50, 1, 10],
            [50, 1, 20],
            [50, 1, 50],
        ],
    },
];

pub(crate) async fn outputs(
    rng: &mut StdRng,
    vested_index: &mut u32,
    coin_type: u32,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut outputs = Vec::new();

    for wallet in STARDUST_MIX {
        let secret_manager = MnemonicSecretManager::try_from_mnemonic(wallet.mnemonic)?;
        for [account_index, internal, address_index] in wallet.addresses {
            let address = secret_manager
                .generate_ed25519_addresses(
                    coin_type,
                    *account_index,
                    *address_index..address_index + 1,
                    if *internal == 1 {
                        Some(GenerateAddressOptions::internal())
                    } else {
                        None
                    },
                )
                .await?[0];

            // Random add up to 2 aliases with foundry and native tokens
            let (alias_foundry_outputs, native_tokens_for_basic_outputs) =
                random_alias_foundry_native_token(address, rng)?;
            let native_tokens_for_nft_outputs = native_tokens_for_basic_outputs.clone();
            outputs.extend(alias_foundry_outputs);

            outputs.push(new_vested_output(
                *vested_index,
                OUTPUT_IOTA_AMOUNT,
                address,
                None,
                rng,
            )?);
            *vested_index -= 1;
            outputs.extend(new_basic_or_nft_outputs(
                OutputBuilder::Basic(BasicOutputBuilder::new_with_amount(OUTPUT_IOTA_AMOUNT)),
                address,
                native_tokens_for_basic_outputs,
                rng,
            )?);
            outputs.extend(new_basic_or_nft_outputs(
                OutputBuilder::Nft(NftOutputBuilder::new_with_amount(
                    OUTPUT_IOTA_AMOUNT,
                    NftId::null(),
                )),
                address,
                native_tokens_for_nft_outputs,
                rng,
            )?);
        }
    }

    Ok(outputs)
}

fn new_basic_or_nft_outputs(
    mut builder: OutputBuilder,
    address: Ed25519Address,
    native_tokens: Vec<NativeToken>,
    rng: &mut StdRng,
) -> anyhow::Result<Vec<(OutputHeader, Output)>> {
    let mut outputs = vec![];

    builder = builder.add_unlock_condition(AddressUnlockCondition::new(address));

    let mut rng_clone = rng.clone();
    let mut add_output_with_unlock_conditions = |unlock_conditions: Vec<UnlockCondition>| {
        let mut new_builder = builder.clone();
        for unlock_condition in unlock_conditions {
            new_builder = new_builder.add_unlock_condition(unlock_condition)
        }
        outputs.push(finish_with_header(new_builder.finish(), &mut rng_clone));
    };

    add_output_with_unlock_conditions(vec![]);
    add_output_with_unlock_conditions(vec![TimelockUnlockCondition::new(rng.gen())?.into()]);
    add_output_with_unlock_conditions(vec![
        ExpirationUnlockCondition::new(address, rng.gen())?.into(),
    ]);
    add_output_with_unlock_conditions(vec![
        StorageDepositReturnUnlockCondition::new(address, STORAGE_DEPOSIT_AMOUNT, u64::MAX)?.into(),
    ]);

    add_output_with_unlock_conditions(vec![
        StorageDepositReturnUnlockCondition::new(
            Ed25519Address::new([0u8; 32]),
            STORAGE_DEPOSIT_AMOUNT,
            u64::MAX,
        )?
        .into(),
    ]);
    add_output_with_unlock_conditions(vec![
        AddressUnlockCondition::new(Ed25519Address::new([0u8; 32])).into(),
        StorageDepositReturnUnlockCondition::new(address, STORAGE_DEPOSIT_AMOUNT, u64::MAX)?.into(),
    ]);
    add_output_with_unlock_conditions(vec![
        TimelockUnlockCondition::new(rng.gen())?.into(),
        ExpirationUnlockCondition::new(address, rng.gen())?.into(),
    ]);
    add_output_with_unlock_conditions(vec![
        TimelockUnlockCondition::new(rng.gen())?.into(),
        ExpirationUnlockCondition::new(address, rng.gen())?.into(),
        StorageDepositReturnUnlockCondition::new(address, STORAGE_DEPOSIT_AMOUNT, u64::MAX)?.into(),
    ]);

    outputs.push(finish_with_header(
        builder
            .with_native_tokens(native_tokens)
            .add_feature(MetadataFeature::new("0")?)
            .add_feature(TagFeature::new("0")?)
            .add_feature(SenderFeature::new(Ed25519Address::new([0u8; 32])))
            .add_immutable_feature(IssuerFeature::new(Ed25519Address::new([0u8; 32])))
            .add_immutable_feature(MetadataFeature::new(
                get_nft_immutable_metadata().to_bytes(),
            )?)
            .finish(),
        rng,
    ));

    Ok(outputs)
}

type NewOutputsWithHeader = Vec<(OutputHeader, Output)>;

fn random_alias_foundry_native_token(
    address: Ed25519Address,
    rng: &mut StdRng,
) -> anyhow::Result<(NewOutputsWithHeader, Vec<NativeToken>)> {
    let mut native_tokens_for_basic_outputs = vec![];
    let mut outputs = vec![];
    for i in 0..rng.gen_range(0..3) {
        let alias_id = AliasId::new(rng.gen());
        let token_scheme = TokenScheme::Simple(SimpleTokenScheme::new(1200, 0, 1200)?);
        let foundry_id = FoundryId::build(&AliasAddress::from(alias_id), 1, token_scheme.kind());
        let token_id = TokenId::from(foundry_id);

        outputs.push(finish_with_header(
            AliasOutputBuilder::new_with_amount(OUTPUT_IOTA_AMOUNT, alias_id)
                .add_feature(SenderFeature::new(address))
                .add_feature(MetadataFeature::new([1, 2, 3])?)
                .add_immutable_feature(IssuerFeature::new(address))
                .add_unlock_condition(StateControllerAddressUnlockCondition::new(address))
                .add_unlock_condition(GovernorAddressUnlockCondition::new(address))
                .with_state_index(1)
                .with_foundry_counter(1)
                .add_native_token(NativeToken::new(token_id, 100)?)
                .finish()?,
            rng,
        ));
        let metadata = Irc30Metadata::new(format!("My Native Token {i}"), "MNT", 10)
            .with_description("A native token for testing");

        outputs.push(finish_with_header(
            FoundryOutputBuilder::new_with_amount(OUTPUT_IOTA_AMOUNT, 1, token_scheme)
                .add_native_token(NativeToken::new(token_id, 100)?)
                .add_unlock_condition(ImmutableAliasAddressUnlockCondition::new(
                    AliasAddress::from(alias_id),
                ))
                .add_immutable_feature(MetadataFeature::try_from(metadata)?)
                .finish()?,
            rng,
        ));

        // Only half the remaining amount as it will be duplicated for nft outputs
        native_tokens_for_basic_outputs.push(NativeToken::new(token_id, 500)?);
    }
    Ok((outputs, native_tokens_for_basic_outputs))
}

fn finish_with_header(builder: impl Into<Output>, rng: &mut StdRng) -> (OutputHeader, Output) {
    (
        OutputHeader::new(
            rng.gen::<[u8; 32]>(),
            random_output_index_with_rng(rng),
            [0; 32],
            MERGE_MILESTONE_INDEX,
            MERGE_TIMESTAMP_SECS,
        ),
        builder.into(),
    )
}

fn get_nft_immutable_metadata() -> Irc27Metadata {
    Irc27Metadata::new(
        "image/svg",
        "https://www.iota.org/logo-icon-dark.svg".parse().unwrap(),
        "OG NFT".to_string(),
    )
    .with_description("The OG NFT.")
    .with_issuer_name("NFT Issuer")
    .with_collection_name("OG")
}

// Outputbuilder we can reuse for basic and NFT outputs
#[derive(Clone)]
enum OutputBuilder {
    Basic(BasicOutputBuilder),
    Nft(NftOutputBuilder),
}

impl OutputBuilder {
    fn add_feature(mut self, feature: impl Into<Feature>) -> Self {
        match self {
            Self::Basic(b) => self = Self::Basic(b.add_feature(feature)),
            Self::Nft(b) => self = Self::Nft(b.add_feature(feature)),
        }
        self
    }
    fn add_immutable_feature(mut self, feature: impl Into<Feature>) -> Self {
        match self {
            Self::Basic(_) => { // Basic outputs can't have immutable features
            }
            Self::Nft(b) => {
                self = Self::Nft(b.add_immutable_feature(feature));
            }
        }
        self
    }
    fn add_unlock_condition(mut self, unlock_condition: impl Into<UnlockCondition>) -> Self {
        match self {
            Self::Basic(b) => {
                self = Self::Basic(b.add_unlock_condition(unlock_condition));
            }
            Self::Nft(b) => {
                self = Self::Nft(b.add_unlock_condition(unlock_condition));
            }
        }
        self
    }
    fn with_native_tokens(mut self, native_tokens: impl IntoIterator<Item = NativeToken>) -> Self {
        match self {
            Self::Basic(b) => {
                self = Self::Basic(b.with_native_tokens(native_tokens));
            }
            Self::Nft(b) => {
                self = Self::Nft(b.with_native_tokens(native_tokens));
            }
        }
        self
    }
    fn finish(self) -> Output {
        match self {
            Self::Basic(b) => b.finish().unwrap().into(),
            Self::Nft(b) => b.finish().unwrap().into(),
        }
    }
}
