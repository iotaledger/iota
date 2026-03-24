import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

const kioskArg = tx.object('<ID>');
const capArg = tx.object('<ID>');
const itemId = tx.pure.id('<ID>');
const itemType = 'ITEM_TYPE';
const priceArg = tx.pure.u64(100_000_000n); // in NANOS (1 IOTA = 10^9 NANOS)

tx.moveCall({
    target: '0x2::kiosk::list',
    arguments: [ kioskArg, capArg, itemId, priceArg ],
    typeArguments: [ itemType ],
});
