import { ConnectButton } from '@iota/dapp-kit';
import { WalletStatus } from './WalletStatus';

function App() {
    return (
        <>
            <div className="sticky top-0 flex px-4 py-2 justify-between items-center border-b border-gray-800 bg-gray-950">
                <h1 className="text-2xl font-bold">dApp Starter Template</h1>
                <ConnectButton />
            </div>
            <div className="max-w-4xl mx-auto">
                <div className="mt-5 pt-2 px-4 bg-gray-900 min-h-[500px] rounded">
                    <WalletStatus />
                </div>
            </div>
        </>
    );
}

export default App;
