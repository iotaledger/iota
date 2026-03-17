import { useIotaClientMutation } from '@iota/dapp-kit';
import { Transaction } from '@iota/iota-sdk/transactions';

function MyComponent() {
    const { mutate } = useIotaClientMutation('dryRunTransactionBlock');

    return (
        <button
            onClick={async () => {
                const tx = new Transaction();
                const bytes = await tx.build();
                mutate({
                    transactionBlock: bytes,
                });
            }}
        >
            Dry run transaction
        </button>
    );
}

export { MyComponent };
