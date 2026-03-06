import { KioskClient, KioskOwnerCap, KioskTransaction } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

const { kioskOwnerCaps } = await kioskClient.getOwnedKiosks({ address: '0x0' });
const cap: KioskOwnerCap = kioskOwnerCaps[0];

async function signAndExecuteTransaction(_args: { tx: Transaction }): Promise<void> {
    // In a real app, use signAndExecuteTransaction from @iota/dapp-kit
}

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
