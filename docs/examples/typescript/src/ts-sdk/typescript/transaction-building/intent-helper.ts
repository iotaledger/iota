import { Commands, Transaction, TransactionObjectInput } from '@iota/iota-sdk/transactions';

function transferToSender(objects: TransactionObjectInput[]) {
    return (tx: Transaction) => {
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
