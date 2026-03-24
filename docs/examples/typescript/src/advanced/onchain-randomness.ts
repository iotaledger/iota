import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();
tx.moveCall({
    target: '<PACKAGE_ID>::example::roll_dice',
    arguments: [tx.object('0x8')],
});
