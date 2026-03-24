import { useIotaClientQuery } from '@iota/dapp-kit';

function MyComponent() {
    const { data, isPending } = useIotaClientQuery('getOwnedObjects', {
        owner: '0x123',
    });

    if (isPending) {
        return <div>Loading...</div>;
    }

    return <pre>{JSON.stringify(data, null, 2)}</pre>;
}

export { MyComponent };
