module stardust::basic_tests{
    use std::option::{is_some};
    use std::type_name::{get, into_string};
    use sui::balance::{Self,Balance,value};
    use sui::sui::SUI;
    use sui::bag::{Self, Bag};
    use sui::coin::{Self,Coin};

    use stardust::basic::{Self};

    use stardust::expiration_unlock_condition;
    use stardust::storage_deposit_return_unlock_condition;
    use stardust::timelock_unlock_condition;

    const EZeroNativeTokenBalance: u64 = 0;
    const ENoBaseTokenBalance: u64 = 1;
    const ENativeTokenBagNonEmpty: u64 = 2;
    const EIotaBalanceMismatch: u64 = 3;

    // one time witness for a coin used in tests
    // we can not declare these inside the test function, and there is no constructor for Basic so we can't test it in a test module
    public struct TEST_A has drop {}
    public struct TEST_B has drop {}

    // demonstration on how to claim the assets from a basic output with all unlock conditions inside one PTB
    #[test]
    fun demonstrate_claiming_ptb() {

        let initial_iota_in_output = 10000;
        let initial_testA_in_output = 100;
        let initial_testB_in_output = 100;
        let sdruc_return_amount = 1000;

        // create a new tx context
        let sender = @0xA;
        let mut ctx = tx_context::new(
            // sender
            sender,
            // tx)hash
            x"3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532",
            // epoch
            1,
            // epoch ts in ms (10 in seconds)
            10000,
            // ids created
            0,
        );

        // mint some tokens
        let sui_balance = balance::create_for_testing<SUI>(initial_iota_in_output);
        let test_a_balance = balance::create_for_testing<TEST_A>(initial_testA_in_output);
        let test_b_balance = balance::create_for_testing<TEST_B>(initial_testB_in_output);

        // add the native token balances to the bag
        let mut native_bag = bag::new(&mut ctx);
        native_bag.add(get<TEST_A>().into_string(), test_a_balance);
        native_bag.add(get<TEST_B>().into_string(), test_b_balance);

        // a basic output in the genesis snapshot
        let basic = basic::create_for_testing(
            sui_balance,
            native_bag,
            option::some(storage_deposit_return_unlock_condition::create_for_testing(@0xB, 1000)),
            option::some(timelock_unlock_condition::create_for_testing(5)),
            option::some(expiration_unlock_condition::create_for_testing(sender, @0xB, 20)),
            &mut ctx,
        );


        // ready with the basic output, now we can claim the assets
        // the task is to assemble a PTB like transaction in move that demonstrates how to claim
        // PTB inputs: basic output ID (`basic`), address to migrate to
        let to = @0xC;

        // command 1: extract the base token and native token bag
        let (extracted_base_token_option, mut native_token_bag) = basic.extract_assets(&mut ctx);
        assert!(extracted_base_token_option.is_some(), ENoBaseTokenBalance);

        // command 2: extract asset A and send to user
        extract_and_send_to<TEST_A>(&mut native_token_bag, to, &mut ctx);

        // TODO: can we actually pass around a mutable reference in a PTB multiple times? Is it consumed after the first use? Do we have to return it and pass it back in the next function?

        // command 3: extract asset B and send to user
        extract_and_send_to<TEST_B>(&mut native_token_bag, to, &mut ctx);

        assert!(native_token_bag.is_empty(), ENativeTokenBagNonEmpty);
        // command 4: delete tha bag
        native_token_bag.destroy_empty();


        // comand 5: create coin from the extracted iota balance
        let iota_coin = create_coin_from_option_balance(extracted_base_token_option, &mut ctx);
        // we should have `initial_iota_in_output` - `sdruc_return_amount` left in the coin
        assert!(iota_coin.value() == (initial_iota_in_output - sdruc_return_amount), EIotaBalanceMismatch);

        // command 6: send back the base token coin to the user
        // if we sponsored the transaction with our own coins, now is the time to detuct it from the user by taking from `iota_coin` and merging it into the gas token
        // since we can dry run the tx before submission, we know how much to charge the user, or we charge the whole gas budget
        transfer::public_transfer(iota_coin, to);

        // !!! migration complete !!! 
    }

    // utility function for the claiming flow that can be called in a PTB
    #[test_only]
    public fun create_coin_from_option_balance<T>(mut b: Option<Balance<T>>, ctx: &mut TxContext) : Coin<T> {
        assert!(b.is_some(), 0);
        let eb = b.extract();
        b.destroy_none();
        coin::from_balance(eb, ctx)
    }

    // get a balance<T> from a bag, and abort if the balance is zero or if there is no balance for the <T>
    #[test_only]
    fun extract<T>(b: &mut Bag) : Balance<T> {
       let key = get<T>().into_string();
       // this will abort if the key doesn't exist
       let nt : Balance<T> = b.remove(key);
       assert!(nt.value() != 0, EZeroNativeTokenBalance);
       nt
    }

    // extract a balance<T> from a bag, create a coin out of it and send it to an address
    #[test_only]
    public fun extract_and_send_to<T>(b: &mut Bag, to: address, ctx: &mut TxContext)  {
        let coin = coin::from_balance(extract<T>(b), ctx);
        transfer::public_transfer(coin, to);
    }


}
