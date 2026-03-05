import { Transaction } from '@iota/iota-sdk/transactions';

let tx = new Transaction();

let itemType = 'ITEM_TYPE';
let itemId = tx.pure.id('<ITEM_ID>');
let kioskArg = tx.object('<ID>');
let capArg = tx.object('<ID>');

let [item, promise] = tx.moveCall({
    target: '0x2::kiosk::borrow_val',
    arguments: [ kioskArg, capArg, itemId ],
    typeArguments: [ itemType ],
});

// Freely mutate or reference the `item`.
// Any calls are available as long as they take a reference.
// `return_val` must be explicitly called.

tx.moveCall({
    target: '0x2::kiosk::return_val',
    arguments: [ kioskArg, item, promise ],
    typeArguments: [ itemType ],
});
