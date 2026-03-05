import { TransferPolicyTransaction } from '@iota/kiosk';

declare const tpTx: TransferPolicyTransaction;

tpTx.withdraw('address_to_transfer_coin', 10_000_000_000n);
