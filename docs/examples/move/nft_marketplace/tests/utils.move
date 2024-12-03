#[test_only]
module nft_marketplace::test_utils {
    use iota::{
        kiosk_test_utils,
        package::Publisher,
        transfer_policy::Self
    };


    public fun create_transfer_policy<T>(sender: address, publisher: &Publisher, ctx: &mut TxContext) {
        let (transfer_policy, policy_cap) = transfer_policy::new<T>(publisher, ctx);
        transfer::public_share_object(transfer_policy);
        transfer::public_transfer(policy_cap, sender);
    }

    public fun create_kiosk(sender: address, ctx: &mut TxContext): ID {
        let (kiosk, kiosk_cap) = kiosk_test_utils::get_kiosk(ctx);
        let kiosk_id = object::id(&kiosk);
        transfer::public_share_object(kiosk);
        transfer::public_transfer(kiosk_cap, sender);

        kiosk_id
}
}