import { Transaction } from '@iota/iota-sdk/transactions';

let tx = new Transaction();
let kioskArg = tx.object('<ID>');
let capArg = tx.object('<ID>');

// because the function uses an Option<u64> argument,
// constructing is a bit more complex
let amountArg = tx.moveCall({
    target: '0x1::option::some',
    arguments: [ tx.pure.u64('<amount>') ],
    typeArguments: [ 'u64' ],
});

// alternatively
let withdrawAllArg = tx.moveCall({
    target: '0x1::option::none',
    typeArguments: [ 'u64' ],
});

let coin = tx.moveCall({
    target: '0x2::kiosk::withdraw',
    arguments: [ kioskArg, capArg, amountArg ],
    typeArguments: [ 'u64' ],
});
