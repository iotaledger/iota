import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();
const kioskArg = tx.object('<ID>');
const capArg = tx.object('<ID>');

// because the function uses an Option<u64> argument,
// constructing is a bit more complex
const amountArg = tx.moveCall({
    target: '0x1::option::some',
    arguments: [ tx.pure.u64(100_000_000n) ],
    typeArguments: [ 'u64' ],
});

// alternatively
const withdrawAllArg = tx.moveCall({
    target: '0x1::option::none',
    typeArguments: [ 'u64' ],
});

const coin = tx.moveCall({
    target: '0x2::kiosk::withdraw',
    arguments: [ kioskArg, capArg, amountArg ],
    typeArguments: [ 'u64' ],
});
