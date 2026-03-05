import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const cap: any;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;

const tx = new Transaction();
const kioskTx = new KioskTransaction({ transaction: tx, kioskClient, cap });

kioskTx
    .withdraw('address_to_transfer_funds', 100000n)
    .finalize();

await signAndExecuteTransaction({ tx });
