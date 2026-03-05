import { Transaction } from '@iota/iota-sdk/transactions';

let tx = new Transaction();

let itemId = tx.pure.id('<ITEM_ID>');
let kioskArg = tx.object('<ID>');
let kioskOwnerCapArg = tx.object('<ID>');

let item = tx.moveCall({
    target: '0x2::kiosk::take',
    arguments: [ kioskArg, kioskOwnerCapArg, itemId ],
    typeArguments: [ '<ITEM_TYPE>' ]
});
