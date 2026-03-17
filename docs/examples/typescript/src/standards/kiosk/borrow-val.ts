import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

const itemType = 'ITEM_TYPE';
const itemId = tx.pure.id('<ITEM_ID>');
const kioskArg = tx.object('<ID>');
const capArg = tx.object('<ID>');

const [item, promise] = tx.moveCall({
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
