import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();
const kioskArg = tx.object('<ID>');
const capArg = tx.object('<ID>');
const itemId = tx.pure.id('<ID>');
const itemType = 'ITEM_TYPE';

tx.moveCall({
    target: '0x2::kiosk::delist',
    arguments: [ kioskArg, capArg, itemId ],
    typeArguments: [ itemType ],
});
