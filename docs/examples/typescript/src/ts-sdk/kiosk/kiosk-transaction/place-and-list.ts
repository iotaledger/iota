import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const cap: any;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;

const item = '0xHeroAddress';
const itemType = '0x..::hero::Hero';

const tx = new Transaction();
const kioskTx = new KioskTransaction({ transaction: tx, kioskClient, cap });

kioskTx
    .placeAndList({
        item,
        itemType,
        price: 100000n,
    })
    .finalize();

await signAndExecuteTransaction({ tx });
