import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();
tx.moveCall({
    target: '0x2::kiosk::default',
});
