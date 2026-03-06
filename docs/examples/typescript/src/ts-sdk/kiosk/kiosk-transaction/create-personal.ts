import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

const kioskClient = new KioskClient({
    client: new IotaClient({ url: getFullnodeUrl(Network.Testnet) }),
    network: Network.Testnet,
});

async function signAndExecuteTransaction(_args: { tx: Transaction }): Promise<void> {
    // In a real app, use signAndExecuteTransaction from @iota/dapp-kit
}

const tx = new Transaction();
const kioskTx = new KioskTransaction({ transaction: tx, kioskClient });

kioskTx
    .createPersonal(true)
    .place({
        itemType: '0x...::hero::Hero',
        item: '0xAHero',
    })
    .finalize();

await signAndExecuteTransaction({ tx });
