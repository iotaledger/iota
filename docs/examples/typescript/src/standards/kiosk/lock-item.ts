import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();

const kioskArg = tx.object('<ID>');
const kioskOwnerCapArg = tx.object('<ID>');
const itemArg = tx.object('<ID>');
const transferPolicyArg = tx.object('<ID>');

tx.moveCall({
    target: '0x2::kiosk::lock',
    arguments: [ kioskArg, kioskOwnerCapArg, transferPolicyArg, itemArg ],
    typeArguments: [ '<ITEM_TYPE>' ],
});
