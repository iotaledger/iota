// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Validity checks of a transaction whose expiration is a valid window.

use iota_protocol_config::{Chain, ProtocolConfig, ProtocolVersion};
use iota_sdk_types::{Address, CheckpointDigest, TransactionExpiration, ValidWindow};

use crate::{
    crypto::{AccountPrivateKey, get_key_pair},
    digests::ChainIdentifier,
    error::{IotaError, IotaResult, UserInputError},
    transaction::{SenderSignedTransactionAPI, TransactionAPI, TxValidityCheckContext},
    utils::{make_transaction_data, to_sender_signed_transaction},
};

const EPOCH: u64 = 10;

/// The latest version on a chain other than testnet and mainnet enables the
/// valid window and, with the P-COOL flow, the lease.
fn config() -> ProtocolConfig {
    ProtocolConfig::get_for_version(ProtocolVersion::MAX, Chain::Unknown)
}

fn genesis_hash() -> CheckpointDigest {
    CheckpointDigest::new([7; 32])
}

/// A window of the current epoch alone, on the network of [`genesis_hash`].
fn window() -> ValidWindow {
    ValidWindow {
        first_valid: EPOCH,
        last_valid: EPOCH,
        genesis_hash: genesis_hash(),
        note: 0,
        lease: None,
    }
}

/// The result of the validity check, in epoch [`EPOCH`] on the network of
/// [`genesis_hash`], of a transaction that expires with `window`.
fn check(config: &ProtocolConfig, window: ValidWindow) -> IotaResult<usize> {
    let (sender, key): (Address, AccountPrivateKey) = get_key_pair();
    let mut data = make_transaction_data(sender);
    *data.expiration_mut_for_testing() = TransactionExpiration::ValidWindow(Box::new(window));
    let transaction = to_sender_signed_transaction(data, &key);

    transaction.data().validity_check(&TxValidityCheckContext {
        config,
        epoch: EPOCH,
        chain_identifier: ChainIdentifier::from(genesis_hash()),
    })
}

fn unsupported(result: IotaResult<usize>) -> bool {
    matches!(
        result,
        Err(IotaError::UserInput {
            error: UserInputError::Unsupported(_)
        })
    )
}

#[test]
fn a_window_of_the_current_epoch_is_accepted() {
    check(&config(), window()).unwrap();
}

#[test]
fn a_window_is_accepted_in_its_first_and_in_its_last_epoch() {
    let config = config();

    check(
        &config,
        ValidWindow {
            last_valid: EPOCH + 1,
            ..window()
        },
    )
    .unwrap();
    check(
        &config,
        ValidWindow {
            first_valid: EPOCH - 1,
            ..window()
        },
    )
    .unwrap();
}

#[test]
fn a_window_that_starts_after_the_current_epoch_is_rejected() {
    let result = check(
        &config(),
        ValidWindow {
            first_valid: EPOCH + 1,
            last_valid: EPOCH + 1,
            ..window()
        },
    );

    assert_eq!(
        result.unwrap_err(),
        IotaError::UserInput {
            error: UserInputError::TransactionNotYetValid {
                first_valid: EPOCH + 1,
                current_epoch: EPOCH,
            }
        }
    );
}

#[test]
fn a_window_that_ended_before_the_current_epoch_is_rejected() {
    let result = check(
        &config(),
        ValidWindow {
            first_valid: EPOCH - 1,
            last_valid: EPOCH - 1,
            ..window()
        },
    );

    assert_eq!(result.unwrap_err(), IotaError::TransactionExpired);
}

#[test]
fn a_window_longer_than_max_txn_life_epochs_is_rejected() {
    let config = config();
    assert_eq!(config.max_txn_life_epochs(), 1);

    let result = check(
        &config,
        ValidWindow {
            first_valid: EPOCH - 1,
            last_valid: EPOCH + 1,
            ..window()
        },
    );

    assert_eq!(
        result.unwrap_err(),
        IotaError::UserInput {
            error: UserInputError::ValidWindowTooLong {
                first_valid: EPOCH - 1,
                last_valid: EPOCH + 1,
                max_txn_life_epochs: 1,
            }
        }
    );
}

#[test]
fn a_window_that_ends_before_it_starts_is_rejected() {
    let result = check(
        &config(),
        ValidWindow {
            first_valid: EPOCH,
            last_valid: EPOCH - 1,
            ..window()
        },
    );

    assert!(matches!(
        result.unwrap_err(),
        IotaError::UserInput {
            error: UserInputError::ValidWindowTooLong { .. }
        }
    ));
}

#[test]
fn a_window_that_names_another_network_is_rejected() {
    let other_network = CheckpointDigest::new([8; 32]);

    let result = check(
        &config(),
        ValidWindow {
            genesis_hash: other_network,
            ..window()
        },
    );

    assert_eq!(
        result.unwrap_err(),
        IotaError::UserInput {
            error: UserInputError::ValidWindowWrongNetwork {
                genesis_hash: other_network,
            }
        }
    );
}

#[test]
fn a_window_is_rejected_while_enable_valid_window_is_off() {
    let mut config = config();
    config.set_enable_valid_window_for_testing(false);

    assert!(unsupported(check(&config, window())));
}

#[test]
fn a_window_is_rejected_on_mainnet_and_testnet() {
    for chain in [Chain::Mainnet, Chain::Testnet] {
        let config = ProtocolConfig::get_for_version(ProtocolVersion::MAX, chain);

        assert!(unsupported(check(&config, window())), "{chain:?}");
    }
}

#[test]
fn a_lease_is_accepted_while_enable_lease_nonce_is_on() {
    check(
        &config(),
        ValidWindow {
            lease: Some([1; 32]),
            ..window()
        },
    )
    .unwrap();
}

#[test]
fn a_lease_is_rejected_while_enable_lease_nonce_is_off() {
    let mut config = config();
    config.set_enable_lease_nonce_for_testing(false);
    let leased = ValidWindow {
        lease: Some([1; 32]),
        ..window()
    };

    assert!(unsupported(check(&config, leased)));
    // The window itself is still accepted.
    check(&config, window()).unwrap();
}

#[test]
fn a_lease_is_rejected_without_the_pcool_flow() {
    let mut config = config();
    config.set_enable_pcool_flow_for_testing(false);
    let leased = ValidWindow {
        lease: Some([1; 32]),
        ..window()
    };

    assert!(unsupported(check(&config, leased)));
}
