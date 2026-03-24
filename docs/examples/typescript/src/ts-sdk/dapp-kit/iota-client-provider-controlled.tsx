import { createNetworkConfig, IotaClientProvider } from '@iota/dapp-kit';
import { getFullnodeUrl } from '@iota/iota-sdk/client';
import { useState } from 'react';

declare function YourApp(): JSX.Element;

// Config options for the networks you want to connect to
const { networkConfig } = createNetworkConfig({
    localnet: { url: getFullnodeUrl('localnet') },
    testnet: { url: getFullnodeUrl('testnet') },
});

function App() {
    const [activeNetwork, setActiveNetwork] = useState<keyof typeof networkConfig>('localnet');

    return (
        <IotaClientProvider
            networks={networkConfig}
            network={activeNetwork}
            onNetworkChange={(network) => {
                setActiveNetwork(network);
            }}
        >
            <YourApp />
        </IotaClientProvider>
    );
}

export default App;
