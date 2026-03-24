import { IotaClientProvider } from '@iota/dapp-kit';
import { getFullnodeUrl, IotaClient, IotaHTTPTransport, type IotaClientOptions } from '@iota/iota-sdk/client';

declare function YourApp(): JSX.Element;

// Config options for the networks you want to connect to
const networks = {
    localnet: { url: getFullnodeUrl('localnet') },
    testnet: { url: getFullnodeUrl('testnet') },
} satisfies Record<string, IotaClientOptions>;

function App() {
    return (
        <IotaClientProvider
            networks={networks}
            defaultNetwork="localnet"
            createClient={(_network, _config) => {
                return new IotaClient({
                    transport: new IotaHTTPTransport({
                        url: 'https://api.safecoin.org',
                        rpc: {
                            headers: {
                                Authorization: 'xyz',
                            },
                        },
                    }),
                });
            }}
        >
            <YourApp />
        </IotaClientProvider>
    );
}

export default App;
