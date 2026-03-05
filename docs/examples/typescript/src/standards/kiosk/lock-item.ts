import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

let kioskArg = tx.object('<ID>');
let kioskOwnerCapArg = tx.object('<ID>');
let itemArg = tx.object('<ID>');
let transferPolicyArg = tx.object('<ID>');

tx.moveCall({
    target: '0x2::kiosk::lock',
    arguments: [ kioskArg, kioskOwnerCapArg, transferPolicyArg, itemArg ],
    typeArguments: [ '<ITEM_TYPE>' ]
});
