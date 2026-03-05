import { Transaction } from '@iota/iota-sdk/transactions';

let tx = new Transaction();
let sender = "0x...";

let [kiosk, kioskOwnerCap] = tx.moveCall({
    target: '0x2::kiosk::new'
});

tx.transferObjects([ kioskOwnerCap ], sender);
tx.moveCall({
    target: '0x2::transfer::public_share_object',
    arguments: [ kiosk ],
    typeArguments: ['0x2::kiosk::Kiosk']
})
