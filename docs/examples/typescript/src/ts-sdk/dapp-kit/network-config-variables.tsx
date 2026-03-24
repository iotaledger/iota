import { createNetworkConfig, IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { getFullnodeUrl } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

declare function YourApp(): JSX.Element;

// Config options for the networks you want to connect to
const { networkConfig, useNetworkVariable } = createNetworkConfig({
    localnet: {
        url: getFullnodeUrl('localnet'),
        variables: {
            myMovePackageId: '0x123',
        },
    },
    testnet: {
        url: getFullnodeUrl('testnet'),
        variables: {
            myMovePackageId: '0x456',
        },
    },
});

const queryClient = new QueryClient();

function App() {
    return (
        <QueryClientProvider client={queryClient}>
            <IotaClientProvider networks={networkConfig} defaultNetwork="localnet">
                <WalletProvider>
                    <YourApp />
                </WalletProvider>
            </IotaClientProvider>
        </QueryClientProvider>
    );
}

function YourAppWithId() {
    const id = useNetworkVariable('myMovePackageId');

    return <div>Package ID: {id}</div>;
}

export { App, YourAppWithId };
