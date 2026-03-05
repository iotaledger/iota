import { TransferPolicyTransaction } from '@iota/kiosk';

declare const tpTx: TransferPolicyTransaction;
declare function percentageToBasisPoints(p: number): number;

tpTx.removeRoyaltyRule().addRoyaltyRule(percentageToBasisPoints(20), 1_000_000_000);
