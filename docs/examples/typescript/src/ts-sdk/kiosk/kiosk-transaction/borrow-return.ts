import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const cap: any;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;

const itemId = '0xHeroAddress';
const itemType = '0x..::hero::Hero';

const tx = new Transaction();
const kioskTx = new KioskTransaction({ transaction: tx, kioskClient, cap });

const [item, promise] = kioskTx.borrow({
    itemId,
    itemType,
});

tx.moveCall({
    target: '0xMyGame::hero::level_up',
    arguments: [item],
});

kioskTx
    .return({
        itemType,
        item,
        promise,
    })
    .finalize();

await signAndExecuteTransaction({ tx });
