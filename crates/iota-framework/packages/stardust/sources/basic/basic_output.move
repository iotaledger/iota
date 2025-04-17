// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module stardust::basic_output;

use iota::bag::Bag;
use iota::balance::Balance;
use iota::transfer::Receiving;
use stardust::expiration_unlock_condition::ExpirationUnlockCondition;
use stardust::storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition;
use stardust::timelock_unlock_condition::TimelockUnlockCondition;

// === Imports ===

// === Structs ===

/// A basic output that has unlock conditions/features.
///   - basic outputs with expiration unlock condition must be a shared object, since that's the only
///     way to handle the two possible addresses that can unlock the output.
///   - notice that there is no `store` ability and there is no custom transfer function:
///       -  you can call `extract_assets`,
///       -  or you can call `receive` in other models to receive a `BasicOutput`.
public struct BasicOutput<phantom T> has key {
    /// Hash of the `outputId` that was migrated.
    id: UID,
    /// The amount of coins held by the output.
    balance: Balance<T>,
    /// The `Bag` holds native tokens, key-ed by the stringified type of the asset.
    /// Example: key: "0xabcded::soon::SOON", value: Balance<0xabcded::soon::SOON>.
    native_tokens: Bag,
    /// The storage deposit return unlock condition.
    storage_deposit_return_uc: Option<StorageDepositReturnUnlockCondition>,
    /// The timelock unlock condition.
    timelock_uc: Option<TimelockUnlockCondition>,
    /// The expiration unlock condition.
    expiration_uc: Option<ExpirationUnlockCondition>,
    /// The metadata feature.
    metadata: Option<vector<u8>>,
    /// The tag feature.
    tag: Option<vector<u8>>,
    /// The sender feature.
    sender: Option<address>,
}

// === Public-Mutative Functions ===

/// Extract the assets stored inside the output, respecting the unlock conditions.
///  - The object will be deleted.
///  - The `StorageDepositReturnUnlockCondition` will return the deposit.
///  - Remaining assets (coins and native tokens) will be returned.
public fun extract_assets<T>(output: BasicOutput<T>, ctx: &mut TxContext): (Balance<T>, Bag) {
    // Unpack the output into its basic part.
    let BasicOutput {
        id,
        balance: mut balance,
        native_tokens,
        storage_deposit_return_uc: mut storage_deposit_return_uc,
        timelock_uc: mut timelock_uc,
        expiration_uc: mut expiration_uc,
        sender: _,
        metadata: _,
        tag: _,
    } = output;

    // If the output has a timelock unlock condition, then we need to check if the timelock_uc has expired.
    if (timelock_uc.is_some()) {
        timelock_uc.extract().unlock(ctx);
    };

    // If the output has an expiration unlock condition, then we need to check who can unlock the output.
    if (expiration_uc.is_some()) {
        expiration_uc.extract().unlock(ctx);
    };

    // If the output has an storage deposit return unlock condition, then we need to return the deposit.
    if (storage_deposit_return_uc.is_some()) {
        storage_deposit_return_uc.extract().unlock(&mut balance, ctx);
    };

    // Destroy the unlock conditions.
    option::destroy_none(timelock_uc);
    option::destroy_none(expiration_uc);
    option::destroy_none(storage_deposit_return_uc);

    // Delete the output.
    object::delete(id);

    return (balance, native_tokens)
}

// === Public-Package Functions ===

/// Utility function to receive a basic output in other stardust modules.
/// Since `BasicOutput` only has `key`, it can not be received via `public_receive`.
/// The private receiver must be implemented in its defining module (here).
/// Other modules in the Stardust package can call this function to receive a basic output (alias, NFT).
public(package) fun receive<T>(
    parent: &mut UID,
    output: Receiving<BasicOutput<T>>,
): BasicOutput<T> {
    transfer::receive(parent, output)
}

// === Test Functions ===

// test only function to create a basic output
#[test_only]
public fun create_for_testing<T>(
    balance: Balance<T>,
    native_tokens: Bag,
    storage_deposit_return_uc: Option<StorageDepositReturnUnlockCondition>,
    timelock_uc: Option<TimelockUnlockCondition>,
    expiration_uc: Option<ExpirationUnlockCondition>,
    metadata: Option<vector<u8>>,
    tag: Option<vector<u8>>,
    sender: Option<address>,
    ctx: &mut TxContext,
): BasicOutput<T> {
    BasicOutput<T> {
        id: object::new(ctx),
        balance,
        native_tokens,
        storage_deposit_return_uc,
        timelock_uc,
        expiration_uc,
        metadata,
        tag,
        sender,
    }
}
