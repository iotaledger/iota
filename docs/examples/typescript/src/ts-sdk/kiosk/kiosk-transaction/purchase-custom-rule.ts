import { KioskClient, KioskTransaction, RuleResolvingParams } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const cap: any;
declare const item: { itemType: string; itemId: string; price: bigint; sellerKiosk: string };
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;
declare const transferRequest: any;

const myCustomRule = {
    rule: `0xMyRuleAddress::game_rule::Rule`,
    packageId: `0xMyRuleAddress`,
    resolveRuleFunction: (params: RuleResolvingParams) => {
        const { transaction, itemType, packageId, extraArgs } = params;
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
