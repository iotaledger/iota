import { Transaction } from '@iota/iota-sdk/transactions';

const txb = new Transaction();
txb.moveCall({
    target: "${PACKAGE_ID}::example::roll_dice",
    arguments: [txb.object('0x8')]
});
