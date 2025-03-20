// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
#[path = "unit_tests/keytool_tests.rs"]
mod keytool_tests;

use std::{
    fmt::{Debug, Display, Formatter},
    path::PathBuf,
    sync::Arc,
};

use anyhow::anyhow;
use aws_config::BehaviorVersion;
use aws_sdk_kms::{
    Client as KmsClient,
    primitives::Blob,
    types::{MessageType, SigningAlgorithmSpec},
};
use bip32::DerivationPath;
use clap::*;
use fastcrypto::{
    ed25519::Ed25519KeyPair,
    encoding::{Base64, Encoding, Hex},
    hash::HashFunction,
    secp256k1::recoverable::Secp256k1Sig,
    traits::{KeyPair, ToFromBytes},
};
use iota_keys::{
    key_derive::generate_new_key,
    keypair_file::{
        read_authority_keypair_from_file, read_keypair_from_file, write_authority_keypair_to_file,
        write_keypair_to_file,
    },
    keystore::{AccountKeystore, Keystore},
};
use iota_types::{
    base_types::IotaAddress,
    crypto::{
        DefaultHash, EncodeDecodeBase64, IotaKeyPair, PublicKey, SignatureScheme,
        get_authority_key_pair,
    },
    error::IotaResult,
    multisig::{MultiSig, MultiSigPublicKey, ThresholdUnit, WeightUnit},
    signature::{GenericSignature, VerifyParams},
    signature_verification::VerifiedDigestCache,
    transaction::{TransactionData, TransactionDataAPI},
};
use json_to_table::{Orientation, json_to_table};
use serde::Serialize;
use serde_json::json;
use shared_crypto::intent::{Intent, IntentMessage};
use tabled::{
    builder::Builder,
    settings::{Modify, Rotate, Width, object::Rows},
};
use tracing::info;

use crate::key_identity::{
    KeyIdentity, get_identity_address_from_keystore, get_identity_alias_from_keystore,
};

#[derive(Subcommand)]
pub enum KeyToolCommand {
    /// Convert private key in Hex or Base64 to new format (Bech32
    /// encoded 33 byte flag || private key starting with "iotaprivkey").
    /// Hex private key format import and export are both deprecated in
    /// IOTA Wallet and IOTA CLI Keystore. Use `iota keytool import` if you
    /// wish to import a key to IOTA Keystore.
    Convert { value: String },
    /// Given a Base64 encoded transaction bytes, decode its components. If a
    /// signature is provided, verify the signature against the transaction
    /// and output the result.
    DecodeOrVerifyTx {
        #[arg(long)]
        tx_bytes: String,
        #[arg(long)]
        sig: Option<GenericSignature>,
        #[arg(long, default_value = "0")]
        cur_epoch: u64,
    },
    /// Given a Base64 encoded MultiSig signature, decode its components.
    /// If tx_bytes is passed in, verify the multisig.
    DecodeMultiSig {
        #[arg(long)]
        multisig: MultiSig,
        #[arg(long)]
        tx_bytes: Option<String>,
        #[arg(long, default_value = "0")]
        cur_epoch: u64,
    },
    /// Output the private key of the given key identity in IOTA CLI Keystore as
    /// Bech32 encoded string starting with `iotaprivkey`.
    Export {
        /// An IOTA address or its alias.
        key_identity: KeyIdentity,
    },
    /// Generate a new keypair with key scheme flag {ed25519 | secp256k1 |
    /// secp256r1} with optional derivation path, default to
    /// m/44'/4218'/0'/0'/0' for ed25519 or m/54'/4218'/0'/0/0 for secp256k1
    /// or m/74'/4218'/0'/0/0 for secp256r1. Word length can be { word12 |
    /// word15 | word18 | word21 | word24} default to word12
    /// if not specified.
    ///
    /// The keypair file is output to the current directory. The content of the
    /// file is a Bech32 encoded string of 33-byte `flag || privkey` or for an
    /// authority a Base64 encoded string of 33-byte formatted as `flag ||
    /// privkey`.
    ///
    /// Use `iota client new-address` if you want to generate and save the key
    /// into iota.keystore.
    Generate {
        key_scheme: SignatureScheme,
        derivation_path: Option<DerivationPath>,
        word_length: Option<String>,
    },
    /// Add a new key to IOTA CLI Keystore using either the input mnemonic
    /// phrase, a Bech32 encoded 33-byte `flag || privkey` starting with
    /// "iotaprivkey" or a seed, the key scheme flag {ed25519 | secp256k1 |
    /// secp256r1} and an optional derivation path, default to
    /// m/44'/4218'/0'/0'/0' for ed25519 or m/54'/4218'/0'/0/0 for secp256k1
    /// or m/74'/4218'/0'/0/0 for secp256r1. Supports mnemonic phrase of
    /// word length 12, 15, 18, 21, 24. Set an alias for the key with the
    /// --alias flag. If no alias is provided, the tool will automatically
    /// generate one.
    Import {
        /// Sets an alias for this address. The alias must start with a letter
        /// and can contain only letters, digits, hyphens (-), or underscores
        /// (_).
        #[arg(long)]
        alias: Option<String>,
        input_string: String,
        key_scheme: SignatureScheme,
        derivation_path: Option<DerivationPath>,
    },
    /// List all keys by its IOTA address, Base64 encoded public key, key scheme
    /// name in iota.keystore.
    List {
        /// Sort by alias
        #[arg(long, short = 's')]
        sort_by_alias: bool,
    },
    /// To MultiSig IOTA Address. Pass in a list of all public keys `flag || pk`
    /// in Base64. See `keytool list` for example public keys.
    MultiSigAddress {
        #[arg(long)]
        threshold: ThresholdUnit,
        #[arg(long, num_args(1..))]
        pks: Vec<PublicKey>,
        #[arg(long, num_args(1..))]
        weights: Vec<WeightUnit>,
    },
    /// Provides a list of participating signatures (`flag || sig || pk` encoded
    /// in Base64), threshold, a list of all public keys and a list of their
    /// weights that define the MultiSig address. Returns a valid MultiSig
    /// signature and its sender address. The result can be used as
    /// signature field for `iota client execute-signed-tx`. The sum
    /// of weights of all signatures must be >= the threshold.
    ///
    /// The order of `sigs` must be the same as the order of `pks`.
    /// e.g. for [pk1, pk2, pk3, pk4, pk5], [sig1, sig2, sig5] is valid, but
    /// [sig2, sig1, sig5] is invalid.
    MultiSigCombinePartialSig {
        #[arg(long, num_args(1..))]
        sigs: Vec<GenericSignature>,
        #[arg(long, num_args(1..))]
        pks: Vec<PublicKey>,
        #[arg(long, num_args(1..))]
        weights: Vec<WeightUnit>,
        #[arg(long)]
        threshold: ThresholdUnit,
    },
    /// Read the content at the provided file path. The accepted format can be
    /// [enum IotaKeyPair] (Base64 encoded of 33-byte `flag || privkey`) or
    /// `type AuthorityKeyPair` (Base64 encoded `privkey`). It prints its
    /// Base64 encoded public key and the key scheme flag.
    Show { file: PathBuf },
    /// Create signature using the private key for the given address (or its
    /// alias) in iota keystore. Any signature commits to a [struct
    /// IntentMessage] consisting of the Base64 encoded of the BCS serialized
    /// transaction bytes itself and its intent. If intent is absent, default
    /// will be used.
    Sign {
        #[arg(long)]
        address: KeyIdentity,
        #[arg(long)]
        data: String,
        #[arg(long)]
        intent: Option<Intent>,
    },
    /// Creates a signature by leveraging AWS KMS. Pass in a key-id to leverage
    /// Amazon KMS to sign a message and the base64 pubkey.
    /// Generate PubKey from pem using iotaledger/base64pemkey
    /// Any signature commits to a [struct IntentMessage] consisting of the
    /// Base64 encoded of the BCS serialized transaction bytes itself and
    /// its intent. If intent is absent, default will be used.
    SignKMS {
        #[arg(long)]
        data: String,
        #[arg(long)]
        keyid: String,
        #[arg(long)]
        intent: Option<Intent>,
        #[arg(long)]
        base64pk: String,
    },
    /// Update an old alias to a new one.
    /// If a new alias is not provided, a random one will be generated.
    UpdateAlias {
        /// An IOTA address or its alias.
        key_identity: KeyIdentity,
        /// The alias must start with a letter and can contain only letters,
        /// digits, dots, hyphens (-), or underscores (_).
        new_alias: Option<String>,
    },
    // Commented for now: https://github.com/iotaledger/iota/issues/1777
    // /// Given the max_epoch, generate an OAuth url, ask user to paste the
    // /// redirect with id_token, call salt server, then call the prover server,
    // /// create a test transaction, use the ephemeral key to sign and execute it
    // /// by assembling to a serialized zkLogin signature.
    // ZkLoginSignAndExecuteTx {
    //     #[arg(long)]
    //     max_epoch: EpochId,
    //     #[arg(long, default_value = "devnet")]
    //     network: String,
    //     #[arg(long, default_value = "false")]
    //     fixed: bool, // if true, use a fixed kp generated from [0; 32] seed.
    //     #[arg(long, default_value = "false")]
    //     test_multisig: bool, // if true, use a multisig address with zklogin and a traditional
    // kp.     #[arg(long, default_value = "false")]
    //     sign_with_sk: bool, /* if true, execute tx with the traditional sig (in the multisig),
    //                          * otherwise with the zklogin sig. */
    // },
    // /// A workaround to the above command because sometimes token pasting does
    // /// not work (for Facebook). All the inputs required here are printed from
    // /// the command above.
    // ZkLoginEnterToken {
    //     #[arg(long)]
    //     parsed_token: String,
    //     #[arg(long)]
    //     max_epoch: EpochId,
    //     #[arg(long)]
    //     jwt_randomness: String,
    //     #[arg(long)]
    //     kp_bigint: String,
    //     #[arg(long)]
    //     ephemeral_key_identifier: IotaAddress,
    //     #[arg(long, default_value = "devnet")]
    //     network: String,
    //     #[arg(long, default_value = "false")]
    //     test_multisig: bool,
    //     #[arg(long, default_value = "false")]
    //     sign_with_sk: bool,
    // },
    // /// Given a zkLogin signature, parse it if valid. If `bytes` provided,
    // /// parse it as either as TransactionData or PersonalMessage based on
    // /// `intent_scope`. It verifies the zkLogin signature based its latest
    // /// JWK fetched. Example request: iota keytool zk-login-sig-verify --sig
    // /// $SERIALIZED_ZKLOGIN_SIG --bytes $BYTES --intent-scope 0 --network devnet
    // /// --curr-epoch 10
    // ZkLoginSigVerify {
    //     /// The Base64 of the serialized zkLogin signature.
    //     #[arg(long)]
    //     sig: String,
    //     /// The Base64 of the BCS encoded TransactionData or PersonalMessage.
    //     #[arg(long)]
    //     bytes: Option<String>,
    //     /// Either 0 for TransactionData or 3 for PersonalMessage.
    //     #[arg(long)]
    //     intent_scope: u8,
    //     /// The current epoch for the network to verify the signature's
    //     /// max_epoch against.
    //     #[arg(long)]
    //     cur_epoch: Option<EpochId>,
    //     /// The network to verify the signature for, determines ZkLoginEnv.
    //     #[arg(long, default_value = "devnet")]
    //     network: String,
    // },
    // /// TESTING ONLY: Generate a fixed ephemeral key and its JWT token with test
    // /// issuer. Produce a zklogin signature for the given data and max epoch.
    // /// e.g. iota keytool zk-login-insecure-sign-personal-message --data "hello"
    // /// --max-epoch 5
    // ZkLoginInsecureSignPersonalMessage {
    //     /// The base64 encoded string of the message to sign, without the intent
    //     /// message wrapping.
    //     #[arg(long)]
    //     data: String,
    //     /// The max epoch used for the zklogin signature validity.
    //     #[arg(long)]
    //     max_epoch: EpochId,
    // },
}

// Command Output types
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AliasUpdate {
    old_alias: String,
    new_alias: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedMultiSig {
    public_base64_key: String,
    sig_base64: String,
    weight: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedMultiSigOutput {
    multisig_address: IotaAddress,
    participating_keys_signatures: Vec<DecodedMultiSig>,
    pub_keys: Vec<MultiSigOutput>,
    threshold: usize,
    sig_verify_result: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodeOrVerifyTxOutput {
    tx: TransactionData,
    result: Option<IotaResult>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Key {
    alias: Option<String>,
    iota_address: IotaAddress,
    public_base64_key: String,
    public_base64_key_with_flag: String,
    key_scheme: String,
    flag: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    mnemonic: Option<String>,
    peer_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedKey {
    exported_private_key: String,
    key: Key,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiSigAddress {
    multisig_address: String,
    multisig: Vec<MultiSigOutput>,
    threshold: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiSigCombinePartialSig {
    multisig_address: IotaAddress,
    multisig_parsed: GenericSignature,
    multisig_serialized: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiSigOutput {
    address: IotaAddress,
    public_base64_key_with_flag: String,
    weight: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertOutput {
    bech32_with_flag: String, // latest IOTA Keystore and IOTA Wallet import/export format
    base64_with_flag: String, // IOTA Keystore storage format
    scheme: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializedSig {
    serialized_sig_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignData {
    iota_address: IotaAddress,
    // Base64 encoded string of serialized transaction data.
    raw_tx_data: String,
    // Intent struct used, see [struct Intent] for field definitions.
    intent: Intent,
    // Base64 encoded [struct IntentMessage] consisting of (intent || message)
    // where message can be `TransactionData` etc.
    raw_intent_msg: String,
    // Base64 encoded blake2b hash of the intent message, this is what the signature commits to.
    digest: String,
    // Base64 encoded `flag || signature || pubkey` for a complete
    // serialized IOTA signature to be send for executing the transaction.
    iota_signature: String,
}

// Commented for now: https://github.com/iotaledger/iota/issues/1777
// #[derive(Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ZkLoginSignAndExecuteTx {
//     tx_digest: String,
// }

// #[derive(Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ZkLoginSigVerifyResponse {
//     data: Option<String>,
//     parsed: Option<String>,
//     jwks: Option<String>,
//     res: Option<IotaResult>,
// }

// #[derive(Serialize)]
// #[serde(rename_all = "camelCase")]
// pub struct ZkLoginInsecureSignPersonalMessage {
//     sig: String,
//     bytes: String,
//     address: String,
// }

#[derive(Serialize)]
#[serde(untagged)]
pub enum CommandOutput {
    Convert(ConvertOutput),
    DecodeMultiSig(DecodedMultiSigOutput),
    DecodeOrVerifyTx(DecodeOrVerifyTxOutput),
    Error(String),
    Export(ExportedKey),
    Generate(Key),
    Import(Key),
    List(Vec<Key>),
    MultiSigAddress(MultiSigAddress),
    MultiSigCombinePartialSig(MultiSigCombinePartialSig),
    Show(Key),
    Sign(SignData),
    SignKMS(SerializedSig),
    UpdateAlias(AliasUpdate),
    // Commented for now: https://github.com/iotaledger/iota/issues/1777
    // ZkLoginSignAndExecuteTx(ZkLoginSignAndExecuteTx),
    // ZkLoginInsecureSignPersonalMessage(ZkLoginInsecureSignPersonalMessage),
    // ZkLoginSigVerify(ZkLoginSigVerifyResponse),
}

impl KeyToolCommand {
    pub async fn execute(self, keystore: &mut Keystore) -> Result<CommandOutput, anyhow::Error> {
        let cmd_result = Ok(match self {
            KeyToolCommand::Convert { value } => {
                let result = convert_private_key_to_bech32(value)?;
                CommandOutput::Convert(result)
            }
            KeyToolCommand::DecodeMultiSig {
                multisig,
                tx_bytes,
                cur_epoch,
            } => {
                let pks = multisig.get_pk().pubkeys();
                let sigs = multisig.get_sigs();
                let bitmap = multisig.get_indices()?;
                let address = IotaAddress::from(multisig.get_pk());

                let pub_keys = pks
                    .iter()
                    .map(|(pk, w)| MultiSigOutput {
                        address: (pk).into(),
                        public_base64_key_with_flag: pk.encode_base64(),
                        weight: *w,
                    })
                    .collect::<Vec<MultiSigOutput>>();

                let threshold = *multisig.get_pk().threshold() as usize;

                let mut output = DecodedMultiSigOutput {
                    multisig_address: address,
                    participating_keys_signatures: vec![],
                    pub_keys,
                    threshold,
                    sig_verify_result: "".to_string(),
                };

                for (sig, i) in sigs.iter().zip(bitmap) {
                    let (pk, w) = pks
                        .get(i as usize)
                        .ok_or(anyhow!("Invalid public keys index".to_string()))?;
                    output.participating_keys_signatures.push(DecodedMultiSig {
                        public_base64_key: pk.encode_base64().clone(),
                        sig_base64: Base64::encode(sig.as_ref()),
                        weight: w.to_string(),
                    })
                }

                if let Some(tx_bytes) = tx_bytes {
                    let tx_bytes = Base64::decode(&tx_bytes)
                        .map_err(|e| anyhow!("Invalid base64 tx bytes: {:?}", e))?;
                    let tx_data: TransactionData = bcs::from_bytes(&tx_bytes)?;
                    let s = GenericSignature::MultiSig(multisig);
                    let res = s.verify_authenticator(
                        &IntentMessage::new(Intent::iota_transaction(), tx_data),
                        address,
                        cur_epoch,
                        &VerifyParams::default(),
                        Arc::new(VerifiedDigestCache::new_empty()),
                    );

                    match res {
                        Ok(()) => output.sig_verify_result = "OK".to_string(),
                        Err(e) => output.sig_verify_result = format!("{:?}", e),
                    };
                };

                CommandOutput::DecodeMultiSig(output)
            }
            KeyToolCommand::DecodeOrVerifyTx {
                tx_bytes,
                sig,
                cur_epoch,
            } => {
                let tx_bytes = Base64::decode(&tx_bytes)
                    .map_err(|e| anyhow!("Invalid base64 key: {:?}", e))?;
                let tx_data: TransactionData = bcs::from_bytes(&tx_bytes)?;
                match sig {
                    None => CommandOutput::DecodeOrVerifyTx(DecodeOrVerifyTxOutput {
                        tx: tx_data,
                        result: None,
                    }),
                    Some(s) => {
                        let res = s.verify_authenticator(
                            &IntentMessage::new(Intent::iota_transaction(), tx_data.clone()),
                            tx_data.sender(),
                            cur_epoch,
                            &VerifyParams::default(),
                            Arc::new(VerifiedDigestCache::new_empty()),
                        );
                        CommandOutput::DecodeOrVerifyTx(DecodeOrVerifyTxOutput {
                            tx: tx_data,
                            result: Some(res),
                        })
                    }
                }
            }
            KeyToolCommand::Export { key_identity } => {
                let address = get_identity_address_from_keystore(key_identity, keystore)?;
                let ikp = keystore.get_key(&address)?;
                let mut key = Key::from(ikp);

                key.alias = keystore.get_alias_by_address(&address).ok();

                let key = ExportedKey {
                    exported_private_key: ikp
                        .encode()
                        .map_err(|_| anyhow!("Cannot decode keypair"))?,
                    key
                };

                CommandOutput::Export(key)
            }
            KeyToolCommand::Generate {
                key_scheme,
                derivation_path,
                word_length,
            } => match key_scheme {
                SignatureScheme::BLS12381 => {
                    let (iota_address, kp) = get_authority_key_pair();
                    let file_name = format!("bls-{iota_address}.key");
                    write_authority_keypair_to_file(&kp, file_name)?;
                    let public_base64_key_with_flag = encode_public_key_with_flag_base64(SignatureScheme::BLS12381.flag(), kp.public().as_ref());
                    CommandOutput::Generate(Key {
                        alias: None,
                        iota_address,
                        public_base64_key: kp.public().encode_base64(),
                        public_base64_key_with_flag,
                        key_scheme: key_scheme.to_string(),
                        flag: SignatureScheme::BLS12381.flag(),
                        mnemonic: None,
                        peer_id: None,
                    })
                }
                _ => {
                    let (iota_address, ikp, _scheme, phrase) =
                        generate_new_key(key_scheme, derivation_path, word_length)?;
                    let file = format!("{iota_address}.key");
                    write_keypair_to_file(&ikp, file)?;
                    let mut key = Key::from(&ikp);
                    key.mnemonic = Some(phrase);
                    CommandOutput::Generate(key)
                }
            },
            KeyToolCommand::Import {
                alias,
                input_string,
                key_scheme,
                derivation_path,
            } => match IotaKeyPair::decode(&input_string) {
                Ok(ikp) => {
                    info!("Importing Bech32 encoded private key to keystore");
                    let mut key = Key::from(&ikp);

                    keystore.add_key(alias, ikp)?;
                    key.alias = Some(keystore.get_alias_by_address(&key.iota_address)?);

                    CommandOutput::Import(key)
                }
                Err(_) => {
                    let iota_address = match Hex::decode(&input_string.replace("0x", "")) {
                        Ok(seed) => {
                            info!("Importing seed to keystore");
                            if seed.len() != 64 {
                                return Err(anyhow!(
                                    "Invalid seed length: {}, only 64 byte seeds are supported",
                                    seed.len()
                                ));
                            }
                            keystore.import_from_seed(&seed, key_scheme, derivation_path, alias)?
                        }
                        Err(_) => {
                            info!("Importing mnemonic to keystore");
                            keystore.import_from_mnemonic(
                                &input_string,
                                key_scheme,
                                derivation_path,
                                alias,
                            )?
                        }
                    };

                    let ikp = keystore.get_key(&iota_address)?;
                    let mut key = Key::from(ikp);

                    key.alias = Some(keystore.get_alias_by_address(&key.iota_address)?);

                    CommandOutput::Import(key)
                }
            },
            KeyToolCommand::List { sort_by_alias } => {
                let mut keys = keystore
                    .keys()
                    .into_iter()
                    .map(|pk| {
                        let mut key = Key::from(pk);
                        key.alias = keystore.get_alias_by_address(&key.iota_address).ok();
                        key
                    })
                    .collect::<Vec<Key>>();
                if sort_by_alias {
                    keys.sort_unstable();
                }
                CommandOutput::List(keys)
            }
            KeyToolCommand::MultiSigAddress {
                threshold,
                pks,
                weights,
            } => {
                let multisig_pk = MultiSigPublicKey::new(pks.clone(), weights.clone(), threshold)?;
                let address: IotaAddress = (&multisig_pk).into();
                let mut output = MultiSigAddress {
                    multisig_address: address.to_string(),
                    multisig: vec![],
                    threshold
                };

                for (pk, w) in pks.into_iter().zip(weights.into_iter()) {
                    output.multisig.push(MultiSigOutput {
                        address: Into::<IotaAddress>::into(&pk),
                        public_base64_key_with_flag: pk.encode_base64(),
                        weight: w,
                    });
                }
                CommandOutput::MultiSigAddress(output)
            }
            KeyToolCommand::MultiSigCombinePartialSig {
                sigs,
                pks,
                weights,
                threshold,
            } => {
                let multisig_pk = MultiSigPublicKey::new(pks, weights, threshold)?;
                let address: IotaAddress = (&multisig_pk).into();
                let multisig = MultiSig::combine(sigs, multisig_pk)?;
                let generic_sig: GenericSignature = multisig.into();
                let multisig_serialized = generic_sig.encode_base64();
                CommandOutput::MultiSigCombinePartialSig(MultiSigCombinePartialSig {
                    multisig_address: address,
                    multisig_parsed: generic_sig,
                    multisig_serialized,
                })
            }
            KeyToolCommand::Show { file } => {
                let res = read_keypair_from_file(&file);
                match res {
                    Ok(ikp) => {
                        let key = Key::from(&ikp);
                        CommandOutput::Show(key)
                    }
                    Err(_) => match read_authority_keypair_from_file(&file) {
                        Ok(keypair) => {
                            let public_base64_key = keypair.public().encode_base64();
                            let public_base64_key_with_flag= encode_public_key_with_flag_base64(SignatureScheme::BLS12381.flag(), keypair.public().as_ref());
                            CommandOutput::Show(Key {
                                alias: None, // alias does not get stored in key files
                                iota_address: (keypair.public()).into(),
                                public_base64_key,
                                public_base64_key_with_flag,
                                key_scheme: SignatureScheme::BLS12381.to_string(),
                                flag: SignatureScheme::BLS12381.flag(),
                                peer_id: None,
                                mnemonic: None,
                            })
                        }
                        Err(e) => CommandOutput::Error(format!(
                            "Failed to read keypair at path {:?}, err: {e}",
                            file
                        )),
                    },
                }
            }
            KeyToolCommand::Sign {
                address,
                data,
                intent,
            } => {
                let address = get_identity_address_from_keystore(address, keystore)?;
                let intent = intent.unwrap_or_else(Intent::iota_transaction);
                let intent_clone = intent.clone();
                let msg: TransactionData =
                    bcs::from_bytes(&Base64::decode(&data).map_err(|e| {
                        anyhow!("Cannot deserialize data as TransactionData {:?}", e)
                    })?)?;
                let intent_msg = IntentMessage::new(intent, msg);
                let raw_intent_msg: String = Base64::encode(bcs::to_bytes(&intent_msg)?);
                let mut hasher = DefaultHash::default();
                hasher.update(bcs::to_bytes(&intent_msg)?);
                let digest = hasher.finalize().digest;
                let iota_signature =
                    keystore.sign_secure(&address, &intent_msg.value, intent_msg.intent)?;
                CommandOutput::Sign(SignData {
                    iota_address: address,
                    raw_tx_data: data,
                    intent: intent_clone,
                    raw_intent_msg,
                    digest: Base64::encode(digest),
                    iota_signature: iota_signature.encode_base64(),
                })
            }
            KeyToolCommand::SignKMS {
                data,
                keyid,
                intent,
                base64pk,
            } => {
                // Currently only supports secp256k1 keys
                let pk_owner = PublicKey::decode_base64(&base64pk)
                    .map_err(|e| anyhow!("Invalid base64 key: {:?}", e))?;
                let address_owner = IotaAddress::from(&pk_owner);
                info!("Address For Corresponding KMS Key: {}", address_owner);
                info!("Raw tx_bytes to execute: {}", data);
                let intent = intent.unwrap_or_else(Intent::iota_transaction);
                info!("Intent: {:?}", intent);
                let msg: TransactionData =
                    bcs::from_bytes(&Base64::decode(&data).map_err(|e| {
                        anyhow!("Cannot deserialize data as TransactionData {:?}", e)
                    })?)?;
                let intent_msg = IntentMessage::new(intent, msg);
                info!(
                    "Raw intent message: {:?}",
                    Base64::encode(bcs::to_bytes(&intent_msg)?)
                );
                let mut hasher = DefaultHash::default();
                hasher.update(bcs::to_bytes(&intent_msg)?);
                let digest = hasher.finalize().digest;
                info!("Digest to sign: {:?}", Base64::encode(digest));

                // Set up the KMS client in default region.
                let config = aws_config::defaults(BehaviorVersion::latest()).load().await;
                let kms = KmsClient::new(&config);

                // Sign the message, normalize the signature and then compacts it
                // serialize_compact is loaded as bytes for Secp256k1Signature
                let response = kms
                    .sign()
                    .key_id(keyid)
                    .message_type(MessageType::Raw)
                    .message(Blob::new(digest))
                    .signing_algorithm(SigningAlgorithmSpec::EcdsaSha256)
                    .send()
                    .await?;
                let sig_bytes_der = response
                    .signature
                    .expect("Requires Asymmetric Key Generated in KMS");

                let mut external_sig = Secp256k1Sig::from_der(sig_bytes_der.as_ref())?;
                external_sig.normalize_s();
                let sig_compact = external_sig.serialize_compact();

                let mut serialized_sig = vec![SignatureScheme::Secp256k1.flag()];
                serialized_sig.extend_from_slice(&sig_compact);
                serialized_sig.extend_from_slice(pk_owner.as_ref());
                let serialized_sig = Base64::encode(&serialized_sig);
                CommandOutput::SignKMS(SerializedSig {
                    serialized_sig_base64: serialized_sig,
                })
            }
            KeyToolCommand::UpdateAlias {
                key_identity,
                new_alias,
            } => {
                let old_alias = get_identity_alias_from_keystore(key_identity, keystore)?;
                let new_alias = keystore.update_alias(&old_alias, new_alias.as_deref())?;
                CommandOutput::UpdateAlias(AliasUpdate {
                    old_alias,
                    new_alias,
                })
            }
            /* Commented for now: https://github.com/iotaledger/iota/issues/1777
               * KeyToolCommand::ZkLoginInsecureSignPersonalMessage { data, max_epoch } => {
               *     let msg = PersonalMessage {
               *         message: data.as_bytes().to_vec(),
               *     };
               *     let sub = "1";
               *     let user_salt = "1";
               *     let intent_msg = IntentMessage::new(Intent::personal_message(),
               * msg.clone()); */

              /*     // set up keypair, nonce with max_epoch
               *     let skp =
               *         IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([0;
               * 32])));     let jwt_randomness = BigUint::from_bytes_be(&[0;
               * 32]).to_string();     let mut eph_pk_bytes = vec![0x00];
               *     eph_pk_bytes.extend(skp.public().as_ref());
               *     let kp_bigint = BigUint::from_bytes_be(&eph_pk_bytes).to_string();
               *     let nonce = get_nonce(&eph_pk_bytes, max_epoch, &jwt_randomness).unwrap(); */

              /*     // call test issuer to get jwt token.
               *     let client = reqwest::Client::new();
               *     let parsed_token = get_test_issuer_jwt_token(
               *         &client,
               *         &nonce,
               *         &OIDCProvider::TestIssuer.get_config().iss,
               *         sub,
               *     )
               *     .await
               *     .unwrap()
               *     .jwt; */

              /*     // call prover-dev for zklogin inputs
               *     let reader = get_proof(
               *         &parsed_token,
               *         max_epoch,
               *         &jwt_randomness,
               *         &kp_bigint,
               *         user_salt,
               *         "https://prover-dev.iota.org/v1",
               *     )
               *     .await
               *     .unwrap();
               *     let (_, aud) = parse_and_validate_jwt(&parsed_token).unwrap();
               *     let address_seed = gen_address_seed(user_salt, "sub", sub, &aud).unwrap();
               *     let zk_login_inputs =
               *         ZkLoginInputs::from_reader(reader, &address_seed.to_string()).unwrap();
               *     let pk = PublicKey::ZkLogin(
               *         ZkLoginPublicIdentifier::new(
               *             zk_login_inputs.get_iss(),
               *             zk_login_inputs.get_address_seed(),
               *         )
               *         .unwrap(),
               *     );
               *     let address = IotaAddress::from(&pk);
               *     // sign with ephemeral key and combine with zklogin inputs to generic
               * signature     let s = Signature::new_secure(&intent_msg, &skp);
               *     let sig = GenericSignature::ZkLoginAuthenticator(ZkLoginAuthenticator::new(
               *         zk_login_inputs,
               *         max_epoch,
               *         s,
               *     ));
               *     CommandOutput::ZkLoginInsecureSignPersonalMessage(
               *         ZkLoginInsecureSignPersonalMessage {
               *             sig: Base64::encode(sig.as_bytes()),
               *             bytes: Base64::encode(data.as_bytes()),
               *             address: address.to_string(),
               *         },
               *     )
               * }
               * KeyToolCommand::ZkLoginSignAndExecuteTx {
               *     max_epoch,
               *     network,
               *     fixed,
               *     test_multisig,
               *     sign_with_sk,
               * } => {
               *     let skp = if fixed {
               *         IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([0;
               * 32])))     } else {
               *         IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut rand::thread_rng()))
               *     };
               *     println!("Ephemeral keypair: {:?}", skp.encode());
               *     let pk = skp.public();
               *     let ephemeral_key_identifier: IotaAddress = (&skp.public()).into();
               *     println!("Ephemeral key identifier: {ephemeral_key_identifier}");
               *     keystore.add_key(None, skp)?; */

              /*     let mut eph_pk_bytes = vec![pk.flag()];
               *     eph_pk_bytes.extend(pk.as_ref());
               *     let kp_bigint = BigUint::from_bytes_be(&eph_pk_bytes);
               *     println!("Ephemeral pubkey (BigInt): {:?}", kp_bigint); */

              /*     let jwt_randomness = if fixed {
               *         "100681567828351849884072155819400689117".to_string()
               *     } else {
               *         let random_bytes = rand::thread_rng().gen::<[u8; 16]>();
               *         let jwt_random_bytes = BigUint::from_bytes_be(&random_bytes);
               *         jwt_random_bytes.to_string()
               *     };
               *     println!("Jwt randomness: {jwt_randomness}");
               *     let url = get_oidc_url(
               *         OIDCProvider::Google,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "25769832374-famecqrhe2gkebt5fvqms2263046lj96.apps.googleusercontent.
               * com",         "https://iota.org/",
               *         &jwt_randomness,
               *     )?;
               *     let url_2 = get_oidc_url(
               *         OIDCProvider::Twitch,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "rs1bh065i9ya4ydvifixl4kss0uhpt",
               *         "https://iota.org/",
               *         &jwt_randomness,
               *     )?;
               *     let url_3 = get_oidc_url(
               *         OIDCProvider::Facebook,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "233307156352917",
               *         "https://iota.org/",
               *         &jwt_randomness,
               *     )?;
               *     let url_4 = get_oidc_url(
               *         OIDCProvider::Kakao,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "aa6bddf393b54d4e0d42ae0014edfd2f",
               *         "https://iota.org/",
               *         &jwt_randomness,
               *     )?;
               *     let url_5 = get_token_exchange_url(
               *         OIDCProvider::Kakao,
               *         "aa6bddf393b54d4e0d42ae0014edfd2f",
               *         "https://iota.org/",
               *         "$YOUR_AUTH_CODE",
               *         "", // not needed
               *     )?;
               *     let url_6 = get_oidc_url(
               *         OIDCProvider::Apple,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "nl.digkas.wallet.client",
               *         "https://iota.org/",
               *         &jwt_randomness,
               *     )?;
               *     let url_7 = get_oidc_url(
               *         OIDCProvider::Slack,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "2426087588661.5742457039348",
               *         "https://iota.org/",
               *         &jwt_randomness,
               *     )?;
               *     let url_8 = get_token_exchange_url(
               *         OIDCProvider::Slack,
               *         "2426087588661.5742457039348",
               *         "https://iota.org/",
               *         "$YOUR_AUTH_CODE",
               *         "39b955a118f2f21110939bf3dff1de90",
               *     )?;
               *     let url_9 = get_oidc_url(
               *         OIDCProvider::AwsTenant((
               *             "us-east-1".to_string(),
               *             "zklogin-example".to_string(),
               *         )),
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "6c56t7re6ekgmv23o7to8r0sic",
               *         "https://www.iota.io/",
               *         &jwt_randomness,
               *     )?;
               *     let url_10 = get_oidc_url(
               *         OIDCProvider::Microsoft,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "2e3e87cb-bf24-4399-ab98-48343d457124",
               *         "https://www.iota.io",
               *         &jwt_randomness,
               *     )?;
               *     let url_11 = get_oidc_url(
               *         OIDCProvider::KarrierOne,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "kns-dev",
               *         "https://iota.org/", // placeholder
               *         &jwt_randomness,
               *     )?;
               *     let url_12 = get_oidc_url(
               *         OIDCProvider::Credenza3,
               *         &eph_pk_bytes,
               *         max_epoch,
               *         "65954ec5d03dba0198ac343a",
               *         "https://example.com/callback",
               *         &jwt_randomness,
               *     )?;
               *     let url_13 = get_oidc_url(
               *         OIDCProvider::AwsTenant(("us-east-1".to_string(),
               * "ambrus".to_string())),         &eph_pk_bytes,
               *         max_epoch,
               *         "t1eouauaitlirg57nove8kvj8",
               *         "https://api.ambrus.studio/callback",
               *         &jwt_randomness,
               *     )?;
               *     println!("Visit URL (Google): {url}");
               *     println!("Visit URL (Twitch): {url_2}");
               *     println!("Visit URL (Facebook): {url_3}");
               *     println!("Visit URL (Kakao): {url_4}");
               *     println!("Token exchange URL (Kakao): {url_5}");
               *     println!("Visit URL (Apple): {url_6}");
               *     println!("Visit URL (Slack): {url_7}");
               *     println!("Token exchange URL (Slack): {url_8}"); */

              /*     println!("Visit URL (AWS): {url_9}");
               *     println!("Visit URL (Microsoft): {url_10}");
               *     println!("Visit URL (KarrierOne): {url_11}");
               *     println!("Visit URL (Credenza3): {url_12}");
               *     println!("Visit URL (AWS - Ambrus): {url_13}"); */

              /*     println!(
               *         "Finish login and paste the entire URL here (e.g. https://iota.org/#id_token=...):"
               *     ); */

              /*     let parsed_token = read_cli_line()?;
               *     let tx_digest = perform_zk_login_test_tx(
               *         &parsed_token,
               *         max_epoch,
               *         &jwt_randomness,
               *         &kp_bigint.to_string(),
               *         ephemeral_key_identifier,
               *         keystore,
               *         &network,
               *         test_multisig,
               *         sign_with_sk,
               *     )
               *     .await?;
               *     CommandOutput::ZkLoginSignAndExecuteTx(ZkLoginSignAndExecuteTx { tx_digest
               * }) }
               * KeyToolCommand::ZkLoginEnterToken {
               *     parsed_token,
               *     max_epoch,
               *     jwt_randomness,
               *     kp_bigint,
               *     ephemeral_key_identifier,
               *     network,
               *     test_multisig,
               *     sign_with_sk,
               * } => {
               *     let tx_digest = perform_zk_login_test_tx(
               *         &parsed_token,
               *         max_epoch,
               *         &jwt_randomness,
               *         &kp_bigint,
               *         ephemeral_key_identifier,
               *         keystore,
               *         &network,
               *         test_multisig,
               *         sign_with_sk,
               *     )
               *     .await?;
               *     CommandOutput::ZkLoginSignAndExecuteTx(ZkLoginSignAndExecuteTx { tx_digest
               * }) }
               * KeyToolCommand::ZkLoginSigVerify {
               *     sig,
               *     bytes,
               *     intent_scope,
               *     cur_epoch,
               *     network,
               * } => {
               *     match GenericSignature::from_bytes(
               *         &Base64::decode(&sig).map_err(|e| anyhow!("Invalid base64 sig: {:?}",
               * e))?,     )? {
               *         GenericSignature::ZkLoginAuthenticator(zk) => {
               *             if bytes.is_none() || cur_epoch.is_none() {
               *                 return
               * Ok(CommandOutput::ZkLoginSigVerify(ZkLoginSigVerifyResponse {
               *                     data: None,
               *                     parsed: Some(serde_json::to_string(&zk)?),
               *                     res: None,
               *                     jwks: None,
               *                 }));
               *             } */

              /*             let client = reqwest::Client::new();
               *             let provider = OIDCProvider::from_iss(zk.get_iss())
               *                 .map_err(|_| anyhow!("Invalid iss"))?;
               *             let jwks = fetch_jwks(&provider, &client).await?;
               *             let parsed: ImHashMap<JwkId, JWK> =
               * jwks.clone().into_iter().collect();             let env = match
               * network.as_str() {                 "devnet" | "localnet" =>
               * ZkLoginEnv::Test,                 "mainnet" | "testnet" =>
               * ZkLoginEnv::Prod,                 _ => return Err(anyhow!("Invalid
               * network")),             };
               *             let verify_params =
               *                 VerifyParams::new(parsed, vec![], env, true, Some(2)); */

              /*             let (serialized, res) = match IntentScope::try_from(intent_scope)
               *                 .map_err(|_| anyhow!("Invalid scope"))?
               *             {
               *                 IntentScope::TransactionData => {
               *                     let tx_data: TransactionData = bcs::from_bytes(
               *                         &Base64::decode(&bytes.unwrap())
               *                             .map_err(|e| anyhow!("Invalid base64 tx data: {:?}",
               * e))?,                     )?; */

              /*                     let sig =
               * GenericSignature::ZkLoginAuthenticator(zk.clone());
               * let res = sig.verify_authenticator(
               * &IntentMessage::new(
               * Intent::iota_transaction(),
               * tx_data.clone(),                         ),
               *                         tx_data.execution_parts().1,
               *                         cur_epoch.unwrap(),
               *                         &verify_params,
               *                         Arc::new(VerifiedDigestCache::new_empty()),
               *                     );
               *                     (serde_json::to_string(&tx_data)?, res)
               *                 }
               *                 IntentScope::PersonalMessage => {
               *                     let data = PersonalMessage {
               *                         message: Base64::decode(&bytes.unwrap()).map_err(|e| {
               *                             anyhow!("Invalid base64 personal message data:
               * {:?}", e)                         })?,
               *                     }; */

              /*                     let sig =
               * GenericSignature::ZkLoginAuthenticator(zk.clone());
               * let res = sig.verify_authenticator(
               * &IntentMessage::new(Intent::personal_message(), data.clone()),
               *                         (&zk).try_into()?,
               *                         cur_epoch.unwrap(),
               *                         &verify_params,
               *                         Arc::new(VerifiedDigestCache::new_empty()),
               *                     );
               *                     (serde_json::to_string(&data)?, res)
               *                 }
               *                 _ => return Err(anyhow!("Invalid intent scope")),
               *             };
               *             CommandOutput::ZkLoginSigVerify(ZkLoginSigVerifyResponse {
               *                 data: Some(serialized),
               *                 parsed: Some(serde_json::to_string(&zk)?),
               *                 jwks: Some(serde_json::to_string(&jwks)?),
               *                 res: Some(res),
               *             })
               *         }
               *         _ => CommandOutput::Error("Not a zkLogin signature".to_string()),
               *     }
               * } */
        });

        cmd_result
    }
}

impl From<&IotaKeyPair> for Key {
    fn from(ikp: &IotaKeyPair) -> Self {
        Key::from(ikp.public())
    }
}

impl From<PublicKey> for Key {
    fn from(pk: PublicKey) -> Self {
        Key {
            alias: None, // this is retrieved later
            iota_address: IotaAddress::from(&pk),
            public_base64_key: Base64::encode(pk.as_ref()),
            public_base64_key_with_flag: pk.encode_base64(),
            key_scheme: pk.scheme().to_string(),
            mnemonic: None,
            flag: pk.flag(),
            peer_id: anemo_styling(&pk),
        }
    }
}

impl Display for CommandOutput {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            // Sign needs to be manually built because we need to wrap the very long
            // rawTxData string and rawIntentMsg strings into multiple rows due to
            // their lengths, which we cannot do with a JsonTable
            CommandOutput::Sign(data) => {
                let intent_table = json_to_table(&json!(&data.intent))
                    .with(tabled::settings::Style::rounded().horizontals([]))
                    .to_string();

                let mut builder = Builder::default();
                builder
                    .set_header([
                        "iotaSignature",
                        "digest",
                        "rawIntentMsg",
                        "intent",
                        "rawTxData",
                        "iotaAddress",
                    ])
                    .push_record([
                        &data.iota_signature,
                        &data.digest,
                        &data.raw_intent_msg,
                        &intent_table,
                        &data.raw_tx_data,
                        &data.iota_address.to_string(),
                    ]);
                let mut table = builder.build();
                table.with(Rotate::Left);
                table.with(tabled::settings::Style::rounded().horizontals([]));
                table.with(Modify::new(Rows::new(0..)).with(Width::wrap(160).keep_words()));
                write!(formatter, "{}", table)
            }
            CommandOutput::UpdateAlias(update) => {
                write!(
                    formatter,
                    "Old alias {} was updated to {}",
                    update.old_alias, update.new_alias
                )
            }
            _ => {
                let json_obj = json![self];
                let mut table = json_to_table(&json_obj);
                let style = tabled::settings::Style::rounded().horizontals([]);
                table.with(style);
                table.array_orientation(Orientation::Column);
                write!(formatter, "{}", table)
            }
        }
    }
}

impl CommandOutput {
    pub fn print(&self, pretty: bool) {
        let line = if pretty {
            format!("{self}")
        } else {
            format!("{:?}", self)
        };
        // Log line by line
        for line in line.lines() {
            // Logs write to a file on the side.  Print to stdout and also log to file, for
            // tests to pass.
            println!("{line}");
            info!("{line}")
        }
    }
}

// when --json flag is used, any output result is transformed into a JSON pretty
// string and sent to std output
impl Debug for CommandOutput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match serde_json::to_string_pretty(self) {
            Ok(json) => write!(f, "{json}"),
            Err(err) => write!(f, "Error serializing JSON: {err}"),
        }
    }
}

/// Converts legacy formatted private key to 33 bytes bech32 encoded private key
/// or vice versa. It can handle:
/// 1) Hex encoded 32 byte private key (assumes scheme is Ed25519), this is the
///    legacy wallet format
/// 2) Base64 encoded 32 bytes private key (assumes scheme is Ed25519)
/// 3) Base64 encoded 33 bytes private key with flag.
/// 4) Bech32 encoded 33 bytes private key with flag.
fn convert_private_key_to_bech32(value: String) -> Result<ConvertOutput, anyhow::Error> {
    let ikp = match IotaKeyPair::decode(&value) {
        Ok(s) => s,
        Err(_) => match Hex::decode(&value) {
            Ok(decoded) => {
                if decoded.len() != 32 {
                    return Err(anyhow!(format!(
                        "Invalid private key length, expected 32 but got {}",
                        decoded.len()
                    )));
                }
                IotaKeyPair::Ed25519(Ed25519KeyPair::from_bytes(&decoded)?)
            }
            Err(_) => match IotaKeyPair::decode_base64(&value) {
                Ok(ikp) => ikp,
                Err(_) => match Ed25519KeyPair::decode_base64(&value) {
                    Ok(kp) => IotaKeyPair::Ed25519(kp),
                    Err(_) => return Err(anyhow!("Invalid private key encoding")),
                },
            },
        },
    };

    Ok(ConvertOutput {
        bech32_with_flag: ikp.encode().map_err(|_| anyhow!("Cannot encode keypair"))?,
        base64_with_flag: ikp.encode_base64(),
        scheme: ikp.public().scheme().to_string(),
    })
}

fn anemo_styling(pk: &PublicKey) -> Option<String> {
    if let PublicKey::Ed25519(public_key) = pk {
        Some(anemo::PeerId(public_key.0).to_string())
    } else {
        None
    }
}

fn encode_public_key_with_flag_base64(flag: u8, public_key: &[u8]) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&[flag]);
    bytes.extend_from_slice(public_key);
    Base64::encode(&bytes[..])
}
