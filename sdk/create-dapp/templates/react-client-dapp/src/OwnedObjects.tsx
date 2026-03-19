import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';

export function OwnedObjects() {
    const account = useCurrentAccount();
    const { data, isPending, error } = useIotaClientQuery(
        'getOwnedObjects',
        {
            owner: account?.address as string,
        },
        {
            enabled: !!account,
        },
    );

    if (!account) {
        return;
    }

    if (error) {
        return <div className="flex">Error: {error.message}</div>;
    }

    if (isPending || !data) {
        return <div className="flex">Loading...</div>;
    }

    return (
        <div className="flex flex-col my-2">
            {data.data.length === 0 ? (
                <p className="text-gray-300">No objects owned by the connected wallet</p>
            ) : (
                <h3 className="text-lg font-semibold">Objects owned by the connected wallet</h3>
            )}
            {data.data.map((object) => (
                <div className="flex" key={object.data?.objectId}>
                    <p className="text-gray-300">Object ID: {object.data?.objectId}</p>
                </div>
            ))}
        </div>
    );
}
