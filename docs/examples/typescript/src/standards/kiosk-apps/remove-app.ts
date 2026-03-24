import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();
const kioskArg = tx.object('<ID>');
const capArg = tx.object('<ID>');

tx.moveCall({
    target: '0x2::kiosk_extension::remove',
    arguments: [ kioskArg, capArg ],
    typeArguments: [ '<letter_box_package>::letterbox_ext::Extension' ],
});
