import { KioskClient, KioskTransaction, TransferPolicyTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;
declare const packageId: string;
declare function percentageToBasisPoints(p: number): number;

const heroPolicyCaps = await kioskClient.getOwnedTransferPoliciesByType({
    type: `${packageId}::hero::Hero`,
    address: '0xConnectedAddress',
});

const tx = new Transaction();
const tpTx = new TransferPolicyTransaction({ kioskClient, transaction: tx, cap: heroPolicyCaps[0] });

tpTx
    .addFloorPriceRule(10n)
    .addLockRule()
    .addRoyaltyRule(percentageToBasisPoints(10), 0)
    .addPersonalKioskRule();

await signAndExecuteTransaction({ tx });
