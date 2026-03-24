import { Transaction } from '@iota/iota-sdk/transactions';

const transaction = new Transaction();

transaction.addSerializationPlugin(async (transactionData, buildOptions, next) => {
    await next();
});
