import { Transaction } from '@iota/iota-sdk/transactions';

let tx = new Transaction();
let kioskArg = tx.object('<ID>');
let capArg = tx.object('<ID>');
let itemId = tx.pure.id('<ID>');
let itemType = 'ITEM_TYPE';

tx.moveCall({
    target: '0x2::kiosk::delist',
    arguments: [ kioskArg, capArg, itemId ],
    typeArguments: [ itemType ]
});
