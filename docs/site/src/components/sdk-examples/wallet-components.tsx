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
import { getDefaultNetwork, getFullnodeUrl } from '@iota/iota-sdk/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useMemo, useState } from 'react';

import '@iota/dapp-kit/dist/index.css';
import React from 'react';

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

const NETWORKS = {
  [getDefaultNetwork()]: { url: getFullnodeUrl(getDefaultNetwork()) },
};

function withProviders(Component: () => React.JSX.Element) {
  return () => {

    if(typeof window === 'undefined') {
      return null
    }

    // Work around server-side pre-rendering
    const queryClient = useMemo(() => new QueryClient(), []);

    return (
      <QueryClientProvider client={queryClient}>
        <IotaClientProvider networks={NETWORKS}>
          <WalletProvider>
            <Component />
          </WalletProvider>
        </IotaClientProvider>
      </QueryClientProvider>
    );
  };
}
