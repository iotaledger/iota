
module conventions::access_control {

  use iota::iota::IOTA;
  use iota::balance::Balance;
  use iota::coin::{Self, Coin};
  use iota::table::{Self, Table};

  public struct Account has key, store {
    id: UID,
    balance: u64
  }

  public struct State has key {
    id: UID,
    accounts: Table<address, u64>,
    balance: Balance<IOTA>
  }

  // ✅ Correct
  // With this function, another protocol can hold the `Account` on behalf of a user.
  public fun withdraw(state: &mut State, account: &mut Account, ctx: &mut TxContext): Coin<IOTA> {
    let authorized_balance = account.balance;

    account.balance = 0;

    coin::take(&mut state.balance, authorized_balance, ctx)
  }

  // ❌ Incorrect
  // This is less composable.
  public fun wrong_withdraw(state: &mut State, ctx: &mut TxContext): Coin<IOTA> {
    let sender = tx_context::sender(ctx);

    let authorized_balance = table::borrow_mut(&mut state.accounts, sender);
    let value = *authorized_balance;
    *authorized_balance = 0;
    coin::take(&mut state.balance, value, ctx)
  }
}
