import { ConnectButton, useCurrentAccount } from '@iota/dapp-kit';
import { isValidIotaObjectId } from '@iota/iota-sdk/utils';
import { useState } from 'react';
import { Counter } from './Counter';
import { CreateCounter } from './CreateCounter';

function App() {
    const currentAccount = useCurrentAccount();
    const [counterId, setCounter] = useState(() => {
        const hash = window.location.hash.slice(1);
        return isValidIotaObjectId(hash) ? hash : null;
    });

    return (
        <>
            <div className="sticky top-0 flex px-4 py-2 justify-between items-center border-b border-gray-800 bg-gray-950">
                <h1 className="text-2xl font-bold">dApp Starter Template</h1>
                <ConnectButton />
            </div>
            <div className="max-w-4xl mx-auto">
                <div className="mt-5 pt-2 px-4 bg-gray-900 min-h-[500px] rounded">
                    {currentAccount ? (
                        counterId ? (
                            <Counter id={counterId} />
                        ) : (
                            <CreateCounter
                                onCreated={(id) => {
                                    window.location.hash = id;
                                    setCounter(id);
                                }}
                            />
                        )
                    ) : (
                        <h2 className="text-xl font-semibold">Please connect your wallet</h2>
                    )}
                </div>
            </div>
        </>
    );
}

export default App;
