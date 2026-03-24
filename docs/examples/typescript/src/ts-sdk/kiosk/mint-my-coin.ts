import { KioskClient, KioskTransaction } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

// In a real app, use signAndExecuteTransaction from @iota/dapp-kit
async function signAndExecuteTransaction(_args: { tx: Transaction }): Promise<void> {}

const packageId = '0x...';
const myType = `${packageId}::my_module::MyStruct<${packageId}::my_coin_module::MyCoin>`;
const otherType = `${packageId}::other_module::OtherStruct<${packageId}::other_coin_module::OtherCoin>`;

const kioskClient = new KioskClient({
    client: new IotaClient({
        url: getFullnodeUrl(Network.Testnet),
    }),
    network: Network.Testnet,
});

async function mintMyCoin(address: string) {
    const { kioskOwnerCaps } = await kioskClient.getOwnedKiosks({ address });

    const cap = kioskOwnerCaps[0];

    const tx = new Transaction();
    const kioskTx = new KioskTransaction({ transaction: tx, kioskClient, cap });

    if (!cap) kioskTx.create();

    tx.moveCall({
        target: `${packageId}::my_module::mint_app::mint`,
        arguments: [kioskTx.getKiosk(), kioskTx.getKioskCap()],
        typeArguments: [myType],
    });

    if (!cap) kioskTx.shareAndTransferCap(address);

    kioskTx.finalize();

    await signAndExecuteTransaction({ tx });
}
