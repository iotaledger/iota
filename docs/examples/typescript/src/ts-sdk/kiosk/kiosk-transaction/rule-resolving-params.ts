import { KioskClient, KioskTransaction, RuleResolvingParams } from '@iota/kiosk';
import { Transaction } from '@iota/iota-sdk/transactions';
import { TransactionArgument } from '@iota/iota-sdk/transactions';

// For reference, here's the RuleResolvingParams contents.
type RuleResolvingParamsRef = {
    transaction: Transaction;
    itemType: string;
    itemId: string;
    price: string;
    policyId: any;
    kiosk: any;
    ownedKiosk: any;
    ownedKioskCap: any;
    transferRequest: TransactionArgument;
    purchasedItem: TransactionArgument;
    packageId: string;
    extraArgs: Record<string, any>;
};
