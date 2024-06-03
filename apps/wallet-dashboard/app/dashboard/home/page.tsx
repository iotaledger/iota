// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import React from 'react';
import {
    useCurrentAccount,
    useCurrentWallet,
    // useAccountList
} from '@mysten/dapp-kit';
import { AccountBalance, AllCoins } from '@/components';
import { useQuery } from '@tanstack/react-query';

function HomeDashboardPage(): JSX.Element {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();

    const { currentWallet } = useCurrentWallet();

    const { data: accountsFull } = useQuery({
        queryKey: [
            { 'account-list-full': 'account-list-full' },
            { 'account-address': account?.address || 'placeholder-account-list' },
            currentWallet?.features['standard:accountList'],
        ],
        queryFn: async () => {
            const accountListFeature = currentWallet?.features['standard:accountList'];

            if (accountListFeature) {
                try {
                    // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                    // @ts-ignore
                    // Attempt to fetch data using the get() method
                    accountListFeature.get().then((f) => {
                        console.log('then works');
                    });
                    // console.log('Account list:', response);
                    return []; // Returning the fetched data
                } catch (error) {
                    console.error('Failed to fetch account list:', error);
                    return []; // Returning an empty array in case of an error
                }
            }
        },
        enabled: !!account?.address,
    });

    React.useEffect(() => {
        (async () => {
            const accountListFeature = currentWallet?.features['standard:accountList'];
            try {
                // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                // @ts-ignore
                const f = await accountListFeature?.get();
                console.log('--- accountListFeature connect', f);
            } catch (error) {
                console.error('Failed to connect the application to the current wallet.', error);
            }
        })();
    }, [currentWallet?.features]);

    return (
        <main className="flex min-h-screen flex-col items-center space-y-8 p-24">
            <p>Connection status: {connectionStatus}</p>
            {connectionStatus === 'connected' && account && (
                <div className="flex flex-col items-center justify-center space-y-2">
                    <h1>Welcome</h1>
                    <div>Address: {account.address}</div>
                    <AccountBalance />
                    <AllCoins />
                </div>
            )}
        </main>
    );
}

export default HomeDashboardPage;
