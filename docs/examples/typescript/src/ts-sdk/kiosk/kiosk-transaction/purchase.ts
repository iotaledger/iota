import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const cap: any;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;

const item = {
    itemType: '0x..::hero::Hero',
    itemId: '0x..',
    price: 100000n,
    sellerKiosk: '0xSellerKiosk',
};

const tx = new Transaction();
const kioskTx = new KioskTransaction({ transaction: tx, kioskClient, cap });

await kioskTx.purchaseAndResolve({
    itemType: item.itemType,
    itemId: item.itemId,
    price: item.price,
    sellerKiosk: item.sellerKiosk,
});

kioskTx.finalize();

await signAndExecuteTransaction({ tx });
