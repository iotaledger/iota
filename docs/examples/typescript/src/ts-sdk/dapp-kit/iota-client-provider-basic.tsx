import { createNetworkConfig, IotaClientProvider } from '@iota/dapp-kit';
import { getFullnodeUrl } from '@iota/iota-sdk/client';

declare function YourApp(): JSX.Element;

// Config options for the networks you want to connect to
const { networkConfig } = createNetworkConfig({
    localnet: { url: getFullnodeUrl('localnet') },
    testnet: { url: getFullnodeUrl('testnet') },
});

function App() {
    return (
        <IotaClientProvider networks={networkConfig} defaultNetwork="localnet">
            <YourApp />
        </IotaClientProvider>
    );
}

export default App;
