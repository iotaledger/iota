import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;

const { kioskOwnerCaps } = await kioskClient.getOwnedKiosks({ address: '0xMyAddress' });

const tx = new Transaction();
const kioskTx = new KioskTransaction({ transaction: tx, kioskClient, cap: kioskOwnerCaps[0] });

kioskTx.withdraw('0xMyAddress', 100_000_000n);

kioskTx
    .place({
        itemType: '0xMyItemType',
        item: '0xMyItem',
    })
    .list({
        itemType: '0xMyItemType',
        itemId: '0xMyItem',
        price: 10000n,
    });

kioskTx.finalize();

await signAndExecuteTransaction({ tx });
