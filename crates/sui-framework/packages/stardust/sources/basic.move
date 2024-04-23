module stardust::basic{
    use std::option::{some,is_some,none,fill};
    use sui::balance::{Balance, destroy_zero, value};
    use sui::sui::SUI;
    use sui::bag::{Bag};
    use sui::transfer::{Receiving};

    use stardust::expiration_unlock_condition::ExpirationUnlockCondition;
    use stardust::storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition;
    use stardust::timelock_unlock_condition::TimelockUnlockCondition;

    // a basic output that has unlock conditions/features
    //   - basic outputs with expiration unlock condition must be a shared object, since that's the only
    //     way to handle the two possible addresses that can unlock the output
    //   - notice that there is no `store` ability and there is no custom transfer function: the only way to interact with the obejct
    public struct Basic has key {
        // hash of the outputId that was migrated
        id: UID,
        // for now IOTA/SMR = SUI bc we use the sui framework
        iota: Balance<SUI>,
        // the bag holds native tokens, key-ed by the stringified type of the asset
        // Example: key: "0xabcded::soon::SOON", value: Balance<0xabcded::soon::SOON>
        tokens: Bag,

        // possible unlock conditions
        storage_deposit_return: Option<StorageDepositReturnUnlockCondition>,
        timelock: Option<TimelockUnlockCondition>,
        expiration: Option<ExpirationUnlockCondition>,

        // possible features
        // they have noeffect and only here to hold data until the object is deleted
        metadata: Option<vector<u8>>,
        tag: Option<vector<u8>>,
        sender: Option<address>
    }

    // extract the assets inside of the output, respecting the unlock conditions
    //  - the object will be deleted
    //  - SDRUC will return the deposit
    //  - remaining assets (iota coins and native tokens) will be returned
    public fun extract_assets(
        // the output to be migrated
        output: Basic,
        ctx: &mut TxContext
    ) : (Option<Balance<SUI>>, Bag) {
        let mut extracted_base_token : Option<Balance<SUI>> = none();

        // unpack the output into its basic part
        let Basic {
            id: id,
            iota: mut iota_balance,
            tokens: tokens,
            // `none` options can be dropped
            storage_deposit_return: mut storage_deposit_return,
            timelock: mut timelock,
            expiration: mut expiration,
            // the features have `drop` so we can just ignore them
            sender: _,
            metadata: _,
            tag: _ } = output;
 
        // if the output has a timelock, then we need to check if the timelock has expired
        if (timelock.is_some()) {
            // extract will make the option `None`
            timelock.extract().unlock(ctx);
        };

        // if the output has an expiration, then we need to check who can unlock the output
        if (expiration.is_some()) {
            // extract will make the option `None`
            expiration.extract().unlock(ctx);
        };

        // if the output has an SDRUC, then we need to return the deposit
        if (storage_deposit_return.is_some()) {
            // extract will make the option `None`
            storage_deposit_return.extract().unlock(&mut iota_balance, ctx);
        };

        // fil lthe return value with the remaining IOTA balance
        let iotas = iota_balance.value();
        if (iotas > 0) {
            extracted_base_token.fill(iota_balance);
        } else {
            iota_balance.destroy_zero();
        };


        // Destroy the output.
        option::destroy_none(timelock);
        option::destroy_none(expiration);
        option::destroy_none(storage_deposit_return);

        object::delete(id);

        // aborts if balance is not zero
        //destroy_zero(iota_balance);

        return (extracted_base_token, tokens)
    }

    // utility function to receive a basic output in other stardust models
    public(package) fun receive(parent: &mut UID, basic: Receiving<Basic>) : Basic {
        transfer::receive<Basic>(parent, basic)
    }

    #[test_only]
    public fun create_for_testing(
        iota: Balance<SUI>,
        tokens: Bag,
        storage_deposit_return: Option<StorageDepositReturnUnlockCondition>,
        timelock: Option<TimelockUnlockCondition>,
        expiration: Option<ExpirationUnlockCondition>,
        ctx: &mut TxContext,
    ): Basic {
        Basic {
            id: object::new(ctx),
            iota,
            tokens,
            storage_deposit_return,
            timelock,
            expiration,
            metadata: some(x"bbbbbbbb"),
            tag: some(x"aaaaaaaa"),
            sender: some(@0xA)
        }
    }

}
