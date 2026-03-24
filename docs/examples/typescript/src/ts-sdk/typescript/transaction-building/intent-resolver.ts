import { Transaction, TransactionDataBuilder, BuildTransactionOptions, Commands, Inputs, type TransactionObjectArgument } from '@iota/iota-sdk/transactions';
import { bcs } from '@iota/iota-sdk/bcs';

const transaction = new Transaction();

transaction.addIntentResolver('TransferToSender', resolveTransferToSender);

async function resolveTransferToSender(
    transactionData: TransactionDataBuilder,
    buildOptions: BuildTransactionOptions,
    next: () => Promise<void>,
) {
    if (!transactionData.sender) {
        throw new Error('Sender must be set to resolve TransferToSender');
    }

    const addressInput = Inputs.Pure(bcs.Address.serialize(transactionData.sender));
    transactionData.inputs.push(addressInput);
    const addressIndex = transactionData.inputs.length - 1;

    for (const [index, transaction] of transactionData.commands.entries()) {
        if (transaction.$kind !== '$Intent' || transaction.$Intent.name !== 'TransferToSender') {
            continue;
        }

        transactionData.replaceCommand(index, [
            Commands.TransferObjects(
                transaction.$Intent.inputs.objects as Extract<
                    TransactionObjectArgument,
                    { $kind: 'Input' }
                >,
                {
                    Input: addressIndex,
                },
            ),
        ]);
    }

    return next();
}
