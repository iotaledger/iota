import { Transaction } from '@iota/iota-sdk/transactions';

let tx = new Transaction();

let kioskArg = tx.object('<ID>');
let capArg = tx.object('<ID>');
let itemId = tx.pure.id('<ID>');
let itemType = 'ITEM_TYPE';
let priceArg = tx.pure.u64('<price>'); // in NANOS (1 IOTA = 10^9 NANOS)

tx.moveCall({
    target: '0x2::kiosk::list',
    arguments: [ kioskArg, capArg, itemId, priceArg ],
    typeArguments: [ itemType ]
});
