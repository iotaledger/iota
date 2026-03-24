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
