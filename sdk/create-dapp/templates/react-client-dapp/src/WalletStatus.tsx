import { useCurrentAccount } from '@iota/dapp-kit';
import { OwnedObjects } from './OwnedObjects';

export function WalletStatus() {
    const account = useCurrentAccount();

    return (
        <div className="my-2">
            <h2 className="text-xl font-semibold mb-2">Wallet Status</h2>

            {account ? (
                <div className="flex flex-col">
                    <p className="text-gray-300">Wallet connected</p>
                    <p className="text-gray-300">Address: {account.address}</p>
                </div>
            ) : (
                <p className="text-gray-300">Wallet not connected</p>
            )}
            <OwnedObjects />
        </div>
    );
}
