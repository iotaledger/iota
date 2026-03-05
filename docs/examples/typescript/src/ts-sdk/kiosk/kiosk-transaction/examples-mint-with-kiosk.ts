import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;

const connectedAddress = '0xAnAddress';

const getCap = async () => {
    let { kioskOwnerCaps } = await kioskClient.getOwnedKiosks({ address: connectedAddress });
    return kioskOwnerCaps[0];
};

const mint = async () => {
    const tx = new Transaction();
    const kioskTx = new KioskTransaction({ kioskClient, transaction: tx, cap: await getCap() });

    let coin = tx.splitCoins(tx.gas, [1_000_000_000]);

    tx.moveCall({
        target: '0xMyGame::hero::mint',
        arguments: [
            coin,
            kioskTx.getKiosk(),
            kioskTx.getKioskCap(),
        ],
    });

    kioskTx.finalize();

    await signAndExecuteTransaction({ tx });
};
