import { Transaction } from '@iota/iota-sdk/transactions';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { getGasSummary } from '../lib/utils';
import {
    AccountsContractMethod,
    CoreContract,
    getHname,
    IscTransaction,
    L2_FROM_L1_GAS_BUDGET,
} from '@iota/isc-sdk';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useNetworkVariables } from '../config';

interface UseBuildL1DepositTransactionProps {
    amount: bigint; // Amount in nanos
    receivingAddress: string;
    refetchInterval?: number;
}

export function useBuildL1DepositTransaction({
    receivingAddress,
    amount,
    refetchInterval,
}: UseBuildL1DepositTransactionProps) {
    const currentAccount = useCurrentAccount();
    const client = useIotaClient();
    const variables = useNetworkVariables();
    const senderAddress = currentAccount?.address as string;
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['l1-deposit-transaction', receivingAddress, amount.toString(), senderAddress],
        queryFn: async () => {
            if (!receivingAddress) {
                throw Error('Invalid input: receivingAddress is missing');
            }

            const amountToPlace = amount + L2_FROM_L1_GAS_BUDGET;

            const iscTx = new IscTransaction(variables.chain);
            const bag = iscTx.newBag();
            const coin = iscTx.coinFromAmount({ amount: amountToPlace });
            iscTx.placeCoinInBag({ coin, bag });
            iscTx.createAndSendToEvm({
                bag,
                transfers: [[IOTA_TYPE_ARG, amount]],
                address: receivingAddress,
                accountsContract: getHname(CoreContract.Accounts),
                accountsFunction: getHname(AccountsContractMethod.TransferAllowanceTo),
            });
            const transaction = iscTx.build();
            transaction.setSender(senderAddress);
            const txBytes = await transaction.build({ client });
            const txDryRun = await client.dryRunTransactionBlock({
                transactionBlock: txBytes,
            });
            if (txDryRun.effects.status.status !== 'success') {
                throw Error(`Tx dry run failed: ${txDryRun.effects.status?.error}`);
            }
            return {
                txBytes,
                txDryRun,
            };
        },
        enabled: !!receivingAddress && !!amount && !!senderAddress && amount > 0n,
        gcTime: 0,
        select: ({ txBytes, txDryRun }) => {
            return {
                transaction: Transaction.from(txBytes),
                gasSummary: getGasSummary(txDryRun),
            };
        },
        refetchInterval,
    });
}
