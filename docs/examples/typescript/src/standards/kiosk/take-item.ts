import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

const itemId = tx.pure.id('<ITEM_ID>');
const kioskArg = tx.object('<ID>');
const kioskOwnerCapArg = tx.object('<ID>');

const item = tx.moveCall({
    target: '0x2::kiosk::take',
    arguments: [ kioskArg, kioskOwnerCapArg, itemId ],
    typeArguments: [ '<ITEM_TYPE>' ],
});
