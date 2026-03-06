import { Commands, Transaction, TransactionObjectInput, TransactionDataBuilder, BuildTransactionOptions } from '@iota/iota-sdk/transactions';

async function resolveTransferToSender(
    _transactionData: TransactionDataBuilder,
    _buildOptions: BuildTransactionOptions,
    next: () => Promise<void>,
): Promise<void> {
    // In a real implementation, resolve the intent here (e.g. set the sender on transfer objects)
    await next();
}

function transferToSender(objects: TransactionObjectInput[]) {
    return (tx: Transaction) => {
        tx.addIntentResolver('TransferToSender', resolveTransferToSender);
        tx.add(
            Commands.Intent({
                name: 'TransferToSender',
                inputs: {
                    objects: objects.map((obj) => tx.object(obj)),
                },
            }),
        );
    };
}

const transaction = new Transaction();
transaction.add(transferToSender(['0x1234']));
