# IOTA dApp Kit

`@iota/dapp-kit` is part of the **IOTA Rebased SDK**, designed specifically for interacting with the IOTA Rebased protocol.

> **Note**: This package is currently supported **only in Testnet and Devnet**, it is **not yet supported in Mainnet**.

The IOTA dApp Kit is a set of React components, hooks, and utilities that make it easy to build a
dApp for the IOTA ecosystem. It provides hooks and components for querying data from the IOTA
blockchain, and connecting to IOTA wallets.

See https://docs.iota.org/ts-sdk/typescript for full documentation

### Core Features

- **Query Hooks:** dApp Kit provides a set of hooks for making rpc calls to the IOTA blockchain,
  making it easy to load any information needed for your dApp.
- **Automatic Wallet State Management:** dApp Kit removes the complexity of state management
  related to wallet connections. You can focus on building your dApp.
- **Supports all IOTA wallets:** No need to manually define wallets you support. All IOTA wallets
  are automatically supported.
- **Easy to integrate:** dApp Kit provides pre-built React Components that you can drop right into
  your dApp, for easier integration
- **Flexible:** dApp Kit ships both fully functional React Component, and lower level hooks that
  you can use to build your own custom components.

## Install from NPM

To use the IOTA dApp Kit in your project, run the following command in your project root:

```sh npm2yarn
npm i --save @iota/dapp-kit @iota/iota-sdk @tanstack/react-query
```

## Setting up providers

To be able to use the hooks and components in the dApp Kit, you need to wrap your app with a couple
providers. The props available on the providers are covered in more detail in their respective docs
pages.

```tsx
import { createNetworkConfig, IotaClientProvider, WalletProvider } from '@iota/dapp-kit';
import { getFullnodeUrl, type IotaClientOptions } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

// Config options for the networks you want to connect to
const { networkConfig } = createNetworkConfig({
    localnet: { url: getFullnodeUrl('localnet') },
    mainnet: { url: getFullnodeUrl('mainnet') },
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
```

## Using UI components to connect to a wallet

The dApp Kit provides a set of flexible UI components that can be used to connect and manage wallet
accounts from your dApp. The components are built on top of
[Radix UI](https://www.radix-ui.com/primitives) and are customizable so you can quickly get your
dApp up and running.

To use our provided UI components, you will need to import the dApp Kit's CSS stylesheet into your
dApp as shown below. For more information regarding customization options, check out the respective
documentation pages for the components and
[themes](https://docs.iota.org/ts-sdk/dapp-kit/themes).

```tsx
import '@iota/dapp-kit/dist/index.css';
```

## Using hooks to make RPC calls

The dApp Kit provides a set of hooks for making RPC calls to the IOTA blockchain. The hooks are thin
wrappers around `useQuery` from `@tanstack/react-query`. For more comprehensive documentation on how
these query hooks can be used, check out the
[react-query docs](https://tanstack.com/query/latest/docs/react/overview).

```tsx
import { useIotaClientQuery } from '@iota/dapp-kit';

function MyComponent() {
    const { data, isPending, error, refetch } = useIotaClientQuery('getOwnedObjects', {
        owner: '0x123',
    });

    if (isPending) {
        return <div>Loading...</div>;
    }

    return <pre>{JSON.stringify(data, null, 2)}</pre>;
}
```
