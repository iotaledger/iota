import { KioskClient, TransferPolicyTransaction } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';

declare const kioskClient: KioskClient;
declare const signAndExecuteTransaction: (args: { tx: Transaction }) => Promise<void>;
declare const heroPackageId: string;
declare function percentageToBasisPoints(p: number): number;

const publisher = '0xPackagePublisherObject';
const tx = new Transaction();

const tpTx = new TransferPolicyTransaction({ kioskClient, transaction: tx });

await tpTx.create({
    type: `${heroPackageId}::hero::Hero`,
    publisher,
});

tpTx.addLockRule()
    .addFloorPriceRule(1000n)
    .addRoyaltyRule(percentageToBasisPoints(10), 100)
    .addPersonalKioskRule()
    .shareAndTransferCap('address_to_transfer_cap_to');

await signAndExecuteTransaction({ tx });
