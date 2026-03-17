import { useIotaClientInfiniteQuery } from '@iota/dapp-kit';

function MyComponent() {
    const { data, isPending, isError, error } =
        useIotaClientInfiniteQuery('getOwnedObjects', {
            owner: '0x123',
        });

    if (isPending) {
        return <div>Loading...</div>;
    }

    if (isError) {
        return <div>Error: {error.message}</div>;
    }

    return <pre>{JSON.stringify(data, null, 2)}</pre>;
}

export { MyComponent };
