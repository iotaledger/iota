import { Transaction } from '@iota/iota-sdk/transactions';

let txb = new Transaction();
let kioskArg = txb.object('<ID>');
let capArg = txb.object('<ID>');

txb.moveCall({
    target: '0x2::kiosk_extension::disable',
    arguments: [ kioskArg, capArg ],
    typeArguments: [ '<letter_box_package>::letterbox_ext::Extension' ]
});
