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
            const iscTx = new IscTransaction(variables.chain);
            const bag = iscTx.newBag();

            const isIotaCoinType = coinType === IOTA_TYPE_ARG;
            // If the coin type is IOTA, we need to add the L2 gas budget to the amount
            const amountToPlace = isIotaCoinType
                ? amount + L2_FROM_L1_GAS_BUDGET
                : L2_FROM_L1_GAS_BUDGET;

            // add iota coins to the bag
            const coin = iscTx.coinFromAmount({ amount: amountToPlace });
            iscTx.placeCoinInBag({ coin, bag });

            // If the coin type is not IOTA, we need to add the coins to the bag
            if (!isIotaCoinType) {
                const totalCoinBalance = coins.reduce((acc, { balance }) => {
                    return BigInt(acc) + BigInt(balance);
                }, BigInt(0));
                const isTransferAllObjects = totalCoinBalance === amount;

                const tx = iscTx.transaction();

                const [primaryCoin, ...mergeCoins] = coins.filter(
                    (coin) => coin.coinType === coinType,
                );
                const primaryCoinInput = tx.object(primaryCoin.coinObjectId);

                if (mergeCoins.length) {
                    tx.mergeCoins(
                        primaryCoinInput,
                        mergeCoins.map((coin) => tx.object(coin.coinObjectId)),
                    );
                }
                const coin = isTransferAllObjects
                    ? primaryCoinInput
                    : tx.splitCoins(primaryCoinInput, [amount]);

                iscTx.placeCoinInBag({
                    bag,
                    coin,
                    coinType,
                });
            }

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
