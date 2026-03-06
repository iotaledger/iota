import { Commands, Transaction } from '@iota/iota-sdk/transactions';

const someId = '0x0';

const transaction = new Transaction();

transaction.add(
    Commands.Intent({
        name: 'TransferToSender',
        inputs: {
            objects: [transaction.object(someId)],
        },
    }),
);
