import {
    BuildTransactionOptions,
    Transaction,
    TransactionDataBuilder,
} from '@iota/iota-sdk/transactions';

const objectCache = new Map<string, { objectId: string; version: string; digest: string }>();

function simpleObjectCachePlugin(
    transactionData: TransactionDataBuilder,
    _options: BuildTransactionOptions,
    next: () => Promise<void>,
) {
    for (const input of transactionData.inputs) {
        if (!input.UnresolvedObject) continue;

        const cached = objectCache.get(input.UnresolvedObject.objectId);

        if (!cached) continue;

        if (cached.version && !input.UnresolvedObject.version) {
            input.UnresolvedObject.version = cached.version;
        }

        if (cached.digest && !input.UnresolvedObject.digest) {
            input.UnresolvedObject.digest = cached.digest;
        }
    }

    return next();
}

const transaction = new Transaction();
transaction.addBuildPlugin(simpleObjectCachePlugin);
