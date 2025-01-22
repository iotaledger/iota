// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// This module contains all the delegated actions. 
/// Like policy, registry and display creation.
/// This is required since the publisher of the tokenized_asset module
/// will not be the same as the publisher of the tokenized asset type.
module asset_tokenization::proxy {

    // IOTA imports
    use iota::package::{Self, Publisher};
    use iota::transfer_policy::{Self, TransferPolicy, TransferPolicyCap};
    use iota::display::{Self, Display};

    // Asset tokenization imports
    use asset_tokenization::tokenized_asset::{TokenizedAsset, PlatformCap};

    const ETypeNotFromPackage: u64 = 1;

    /// OTW used to claim the publisher.
    public struct PROXY has drop {}

    /// A shared object used to hold the publisher object
    /// and limit who accesses and creates Transfer Policies for Tokenized Assets. 
    public struct Registry has key {
        id: UID,
        publisher: Publisher
    }

    /// A shared object used to house the empty transfer policy.
    /// Need to create one per type T of Tokenized Asset.
    public struct ProtectedTP<phantom T> has key, store {
        id: UID,
        policy_cap: TransferPolicyCap<T>,
        transfer_policy: TransferPolicy<T>
    }

    /// Creates the Publisher object, wraps it inside the Registry and shares the Registry object.
    fun init(otw: PROXY, ctx: &mut TxContext) {
        let registry = Registry {
            id: object::new(ctx),
            publisher: package::claim(otw, ctx)
        };

        transfer::share_object(registry);
    }

    /// Uses the Publisher that is nested inside the registry along with the sender's Publisher
    /// to create a Transfer Policy for the type TokenizedAsset<T>,
    /// where T matches with the Publisher object.
    public fun setup_tp<T: drop>(
        registry: &Registry,
        publisher: &Publisher,
        ctx: &mut TxContext
    ): (TransferPolicy<TokenizedAsset<T>>, TransferPolicyCap<TokenizedAsset<T>>) {
        assert!(package::from_package<T>(publisher), ETypeNotFromPackage);
        
        // Creates an empty TP and shares a ProtectedTP<T> object.
        // This can be used to bypass the lock rule under specific conditions.
        // Storing inside the cap the ProtectedTP with no way to access it
        // as we do not want to modify this policy
        let (transfer_policy, cap) = transfer_policy::new<TokenizedAsset<T>>(&registry.publisher, ctx);
        let protected_tp = ProtectedTP {
            transfer_policy,
            policy_cap: cap,
            id: object::new(ctx)
        };
        transfer::share_object(protected_tp);

        transfer_policy::new<TokenizedAsset<T>>(&registry.publisher, ctx)
    }

    /// Uses the Publisher that is nested inside the registry along with the sender's Publisher
    /// to create and return an empty Display for the type TokenizedAsset<T>,
    /// where T matches with the Publisher object.
    public fun new_display<T: drop>(
        registry: &Registry,
        publisher: &Publisher,
        ctx: &mut TxContext
    ): Display<TokenizedAsset<T>> {
        assert!(package::from_package<T>(publisher), ETypeNotFromPackage);
        display::new<TokenizedAsset<T>>(&registry.publisher, ctx)
    }

    /// Returns the Transfer Policy for the type TokenizedAsset<T>.
    public(package) fun transfer_policy<T>(protected_tp: &ProtectedTP<T>): &TransferPolicy<T> {
        &protected_tp.transfer_policy
    }

    /// A way for the platform to access the publisher mutably.
    public fun publisher_mut(_: &PlatformCap, registry: &mut Registry): &mut Publisher {
        &mut registry.publisher
    }

    #[test_only]
    public fun test_registry(ctx: &mut TxContext): Registry{
        Registry {
            id: object::new(ctx),
            publisher: package::claim(PROXY {}, ctx)
        }
    }

    #[test_only]
    public fun test_init(ctx: &mut TxContext) {
        init(PROXY {}, ctx);
    }

    #[test_only]
    public fun test_burn_registry(registry: Registry) {
        let Registry {id, publisher} = registry;
        package::burn_publisher(publisher);
        object::delete(id);
    }
}
