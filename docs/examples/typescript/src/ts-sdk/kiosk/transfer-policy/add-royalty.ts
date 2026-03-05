import { TransferPolicyTransaction } from '@iota/kiosk';

declare const tpTx: TransferPolicyTransaction;
declare function percentageToBasisPoints(p: number): number;

tpTx.addRoyaltyRule(percentageToBasisPoints(30), 1_000_000_000);
