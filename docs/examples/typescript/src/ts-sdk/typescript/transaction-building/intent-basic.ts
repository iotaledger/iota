import { Commands, Transaction } from '@iota/iota-sdk/transactions';

declare const someId: string;

const transaction = new Transaction();

transaction.add(
    Commands.Intent({
        name: 'TransferToSender',
        inputs: {
            objects: [transaction.object(someId)],
        },
    }),
);
