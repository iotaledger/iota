import { Transaction } from '@iota/iota-sdk/transactions';
import { useCurrentAccount, useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import { createL1DepositTransaction, getGasSummary } from '../lib/utils';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useNetworkVariables } from '../config';
import { CoinStruct } from '@iota/iota-sdk/client';

interface UseBuildL1DepositTransactionProps {
    amount: bigint; // Amount in nanos
    receivingAddress: string;
    coins: CoinStruct[];
    coinType?: string;
    refetchInterval?: number;
}

export function useBuildL1DepositTransaction({
    receivingAddress,
    amount,
    coins,
    coinType = IOTA_TYPE_ARG,
    refetchInterval,
}: UseBuildL1DepositTransactionProps) {
    const senderAddress = useCurrentAccount()?.address as string;
    const client = useIotaClient();
    const variables = useNetworkVariables();
    return useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: ['l1-deposit-transaction', receivingAddress, amount.toString(), senderAddress],
        queryFn: async () => {
            if (!receivingAddress) {
                throw Error('Invalid input: receivingAddress is missing');
            }

            const transaction = createL1DepositTransaction({
                amount,
                receivingAddress,
                coins,
                coinType,
                chain: variables.chain,
            });

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
