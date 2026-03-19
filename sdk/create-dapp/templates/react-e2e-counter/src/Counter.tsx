import {
    useCurrentAccount,
    useSignAndExecuteTransaction,
    useIotaClient,
    useIotaClientQuery,
} from '@iota/dapp-kit';
import type { IotaObjectData } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { useNetworkVariable } from './networkConfig';
import { useState } from 'react';

export function Counter({ id }: { id: string }) {
    const counterPackageId = useNetworkVariable('counterPackageId');
    const iotaClient = useIotaClient();
    const currentAccount = useCurrentAccount();
    const { mutate: signAndExecute } = useSignAndExecuteTransaction();
    const { data, isPending, error, refetch } = useIotaClientQuery('getObject', {
        id,
        options: {
            showContent: true,
            showOwner: true,
        },
    });

    const [waitingForTxn, setWaitingForTxn] = useState('');

    const executeMoveCall = (method: 'increment' | 'reset') => {
        setWaitingForTxn(method);

        const tx = new Transaction();

        if (method === 'reset') {
            tx.moveCall({
                arguments: [tx.object(id), tx.pure.u64(0)],
                target: `${counterPackageId}::counter::set_value`,
            });
        } else {
            tx.moveCall({
                arguments: [tx.object(id)],
                target: `${counterPackageId}::counter::increment`,
            });
        }

        signAndExecute(
            {
                transaction: tx,
            },
            {
                onSuccess: (tx) => {
                    iotaClient.waitForTransaction({ digest: tx.digest }).then(async () => {
                        await refetch();
                        setWaitingForTxn('');
                    });
                },
            },
        );
    };

    if (isPending) return <p className="text-gray-300">Loading...</p>;

    if (error) return <p className="text-red-400">Error: {error.message}</p>;

    if (!data.data) return <p className="text-gray-300">Not found</p>;

    const ownedByCurrentAccount = getCounterFields(data.data)?.owner === currentAccount?.address;

    return (
        <>
            <h3 className="text-lg font-semibold">Counter {id}</h3>

            <div className="flex flex-col gap-2">
                <p className="text-gray-300">Count: {getCounterFields(data.data)?.value}</p>
                <div className="flex flex-row gap-2">
                    <button
                        className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
                        onClick={() => executeMoveCall('increment')}
                        disabled={waitingForTxn !== ''}
                    >
                        {waitingForTxn === 'increment' ? 'Loading...' : 'Increment'}
                    </button>
                    {ownedByCurrentAccount ? (
                        <button
                            className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
                            onClick={() => executeMoveCall('reset')}
                            disabled={waitingForTxn !== ''}
                        >
                            {waitingForTxn === 'reset' ? 'Loading...' : 'Reset'}
                        </button>
                    ) : null}
                </div>
            </div>
        </>
    );
}

function getCounterFields(data: IotaObjectData) {
    if (data.content?.dataType !== 'moveObject') {
        return null;
    }

    return data.content.fields as { value: number; owner: string };
}
