import { TransferPolicyTransaction } from '@iota/kiosk';

declare const tpTx: TransferPolicyTransaction;

tpTx.addFloorPriceRule(10_000_000_000n);
