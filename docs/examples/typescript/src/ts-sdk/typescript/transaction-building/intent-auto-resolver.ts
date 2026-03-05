import { Commands, Transaction, TransactionObjectInput, TransactionDataBuilder, BuildTransactionOptions } from '@iota/iota-sdk/transactions';

declare function resolveTransferToSender(
    transactionData: TransactionDataBuilder,
    buildOptions: BuildTransactionOptions,
    next: () => Promise<void>,
): Promise<void>;

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
