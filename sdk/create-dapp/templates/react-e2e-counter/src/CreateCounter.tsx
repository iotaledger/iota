import { Transaction } from '@iota/iota-sdk/transactions';
import { useSignAndExecuteTransaction, useIotaClient } from '@iota/dapp-kit';
import { useNetworkVariable } from './networkConfig';

export function CreateCounter({ onCreated }: { onCreated: (id: string) => void }) {
    const counterPackageId = useNetworkVariable('counterPackageId');
    const iotaClient = useIotaClient();
    const { mutate: signAndExecute, isSuccess, isPending } = useSignAndExecuteTransaction();

    function create() {
        const tx = new Transaction();

        tx.moveCall({
            arguments: [],
            target: `${counterPackageId}::counter::create`,
        });

        signAndExecute(
            {
                transaction: tx,
            },
            {
                onSuccess: async ({ digest }) => {
                    const { effects } = await iotaClient.waitForTransaction({
                        digest: digest,
                        options: {
                            showEffects: true,
                        },
                    });

                    const objectId = effects?.created?.[0]?.reference?.objectId;
                    if (objectId) {
                        onCreated(objectId);
                    } else {
                        console.error('Failed to get objectId from transaction effects');
                    }
                },
            },
        );
    }

    return (
        <div>
            <button
                className="px-6 py-3 text-lg bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
                onClick={() => {
                    create();
                }}
                disabled={isSuccess || isPending}
            >
                {isSuccess || isPending ? 'Creating...' : 'Create Counter'}
            </button>
        </div>
    );
}
