import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

const itemArg = tx.object('<ID>');
const kioskArg = tx.object('<ID>');
const kioskOwnerCapArg = tx.object('<ID>');

tx.moveCall({
    target: '0x2::kiosk::place',
    arguments: [ kioskArg, kioskOwnerCapArg, itemArg ],
    typeArguments: [ '<ITEM_TYPE>' ],
});
