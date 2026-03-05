import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;

const tx = new Transaction();
const kioskTx = new KioskTransaction({ transaction: tx, kioskClient });

kioskTx.create();

kioskTx.place({
    itemType: '0x...::hero::Hero',
    item: '0xAHero',
});

kioskTx.shareAndTransferCap('0xMyAddress');

kioskTx.finalize();

await signAndExecuteTransaction({ tx });
