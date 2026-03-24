import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

const { kioskOwnerCaps } = await kioskClient.getOwnedKiosks({ address: '0x0' });
const cap = kioskOwnerCaps[0];

async function signAndExecuteTransaction(_args: { tx: Transaction }): Promise<void> {
    // In a real app, use signAndExecuteTransaction from @iota/dapp-kit
}

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
