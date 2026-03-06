import { KioskClient, KioskTransaction, RuleResolvingParams } from '@iota/kiosk';
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

const item = {
    itemType: '0x..::hero::Hero',
    itemId: '0x..',
    price: 100000n,
    sellerKiosk: '0xSellerKiosk',
};

const myCustomRule = {
    rule: `0xMyRuleAddress::game_rule::Rule`,
    packageId: `0xMyRuleAddress`,
    resolveRuleFunction: (params: RuleResolvingParams) => {
        const { transaction, transferRequest, itemType, packageId, extraArgs } = params;
        const { gamePass } = extraArgs;
        if (!gamePass) throw new Error('GamePass not supplied');

        transaction.moveCall({
            target: `${packageId}::game_rule::prove_pass`,
            typeArguments: [itemType],
            arguments: [transferRequest, transaction.object(gamePass)],
        });
    },
};

kioskClient.addRuleResolver(myCustomRule);

const tx = new Transaction();
const kioskTx = new KioskTransaction({ transaction: tx, kioskClient, cap });

await kioskTx.purchaseAndResolve({
    itemType: item.itemType,
    itemId: item.itemId,
    price: item.price,
    sellerKiosk: item.sellerKiosk,
    extraArgs: {
        gamePass: '0xMyGamePassObjectId',
    },
});

kioskTx.finalize();

await signAndExecuteTransaction({ tx });
