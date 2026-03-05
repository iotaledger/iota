import { KioskClient, KioskOwnerCap, KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const cap: KioskOwnerCap;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;

const levelUp = async (object: { itemType: string; itemId: string }) => {
    const tx = new Transaction();

    const kioskTx = new KioskTransaction({ kioskClient, transaction: tx, cap });
    kioskTx.borrowTx(object, (item) => {
        tx.moveCall({
            target: '0xMyGame::hero::level_up',
            arguments: [item],
        });
    });
    kioskTx.finalize();

    await signAndExecuteTransaction({ tx });
};

levelUp({
    itemType: '0x2MyGame::hero::Hero',
    itemId: '0xMyHeroObjectId',
});
