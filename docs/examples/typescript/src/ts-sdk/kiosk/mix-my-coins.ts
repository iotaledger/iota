import { KioskClient, KioskTransaction, type KioskOwnerCap } from '@iota/kiosk';
import { IotaClient, Network, getFullnodeUrl } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';

declare function signAndExecuteTransaction(args: { tx: Transaction }): Promise<void>;

const packageId = '0x...';
const myType = `${packageId}::my_module::MyStruct<${packageId}::my_coin_module::MyCoin>`;

const kioskClient = new KioskClient({
    client: new IotaClient({
        url: getFullnodeUrl(Network.Testnet),
    }),
    network: Network.Testnet,
});

async function mixMyCoins(firstCoinObjectId: string, secondCoinObjectId: string, cap: KioskOwnerCap) {
    const tx = new Transaction();
    const kioskTx = new KioskTransaction({ transaction: tx, kioskClient, cap });

    const [coin1, promise1] = kioskTx.borrow({
        itemType: myType,
        itemId: firstCoinObjectId,
    });

    const [coin2, promise2] = kioskTx.borrow({
        itemType: myType,
        itemId: secondCoinObjectId,
    });

    tx.moveCall({
        target: `${packageId}::mix_app::mix`,
        arguments: [
            coin1,
            coin2,
            kioskTx.getKiosk(),
            kioskTx.getKioskCap(),
        ],
        typeArguments: [myType],
    });

    kioskTx.return({
        itemType: myType,
        item: coin1,
        promise: promise1
    })
    .return({
        itemType: myType,
        item: coin2,
        promise: promise2
    }).finalize();

    await signAndExecuteTransaction({ tx });
}
