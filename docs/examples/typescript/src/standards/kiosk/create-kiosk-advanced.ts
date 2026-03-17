import { Transaction } from '@iota/iota-sdk/transactions';

const tx = new Transaction();
const sender = '0x...';

const [kiosk, kioskOwnerCap] = tx.moveCall({
    target: '0x2::kiosk::new',
});

tx.transferObjects([ kioskOwnerCap ], sender);
tx.moveCall({
    target: '0x2::transfer::public_share_object',
    arguments: [ kiosk ],
    typeArguments: ['0x2::kiosk::Kiosk'],
});
