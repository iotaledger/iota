import { Transaction } from '@iota/iota-sdk/transactions';

let tx = new Transaction();

let itemArg = tx.object('<ID>');
let kioskArg = tx.object('<ID>');
let kioskOwnerCapArg = tx.object('<ID>');

tx.moveCall({
    target: '0x2::kiosk::place',
    arguments: [ kioskArg, kioskOwnerCapArg, itemArg ],
    typeArguments: [ '<ITEM_TYPE>' ]
})
