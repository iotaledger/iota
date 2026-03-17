import { Transaction, TransactionArgument } from '@iota/iota-sdk/transactions';

// For reference, here's the RuleResolvingParams contents.
type RuleResolvingParamsRef = {
    transaction: Transaction;
    itemType: string;
    itemId: string;
    price: string;
    policyId: unknown;
    kiosk: unknown;
    ownedKiosk: unknown;
    ownedKioskCap: unknown;
    transferRequest: TransactionArgument;
    purchasedItem: TransactionArgument;
    packageId: string;
    extraArgs: Record<string, unknown>;
};
