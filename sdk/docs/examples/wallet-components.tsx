// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ConnectButton,
    ConnectModal,
    IotaClientProvider,
    useCurrentAccount,
    WalletProvider,
} from '@iota/dapp-kit';
import { getFullnodeUrl } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useState } from 'react';

import '@iota/dapp-kit/dist/index.css';

export const ConnectButtonExample = withProviders(() => {
    return <ConnectButton />;
});

export const ControlledConnectModalExample = withProviders(() => {
    const currentAccount = useCurrentAccount();
    const [open, setOpen] = useState(false);

    return (
        <ConnectModal
            trigger={
                <button disabled={!!currentAccount}>
                    {' '}
                    {currentAccount ? 'Connected' : 'Connect'}
                </button>
            }
            open={open}
            onOpenChange={(isOpen) => setOpen(isOpen)}
        />
    );
});

export const UncontrolledConnectModalExample = withProviders(() => {
    const currentAccount = useCurrentAccount();

    return (
        <ConnectModal
            trigger={
                <button disabled={!!currentAccount}>
                    {' '}
                    {currentAccount ? 'Connected' : 'Connect'}
                </button>
            }
        />
    );
});

function withProviders(Component: React.FunctionComponent<object>) {
    // Work around server-side pre-rendering
    const queryClient = new QueryClient();
    const networks = {
        mainnet: { url: getFullnodeUrl('mainnet') },
    };

    return () => {
        const [shouldRender, setShouldRender] = useState(false);
        useEffect(() => {
            setShouldRender(true);
        }, [setShouldRender]);

        if (!shouldRender) {
            return null;
        }

        return (
            <QueryClientProvider client={queryClient}>
                <IotaClientProvider networks={networks}>
                    <WalletProvider
                        stashedWallet={{
                            name: 'dApp Kit Docs',
                        }}
                    >
                        <Component />
                    </WalletProvider>
                </IotaClientProvider>
            </QueryClientProvider>
        );
    };
}
