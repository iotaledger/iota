import { useIotaClientQueries } from '@iota/dapp-kit';

function MyComponent() {
    const { data, isPending, isError } = useIotaClientQueries({
        queries: [
            {
                method: 'getAllBalances',
                params: {
                    owner: '0x123',
                },
            },
            {
                method: 'queryTransactionBlocks',
                params: {
                    filter: {
                        FromAddress: '0x123',
                    },
                },
            },
        ],
        combine: (result) => {
            return {
                data: result.map((res) => res.data),
                isSuccess: result.every((res) => res.isSuccess),
                isPending: result.some((res) => res.isPending),
                isError: result.some((res) => res.isError),
            };
        },
    });

    if (isPending) {
        return <div>Loading...</div>;
    }

    if (isError) {
        return <div>Fetching Error</div>;
    }

    return <pre>{JSON.stringify(data, null, 2)}</pre>;
}

export { MyComponent };
