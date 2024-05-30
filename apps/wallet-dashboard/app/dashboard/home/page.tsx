// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { useCurrentAccount, useCurrentWallet, useAccountList } from '@mysten/dapp-kit';
import { AccountBalance, AllCoins } from '@/components';
import { useQuery } from '@tanstack/react-query';

function HomeDashboardPage(): JSX.Element {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();
    const { data: accounts } = useAccountList();
    console.log('--- accounts', accounts);
    const { currentWallet } = useCurrentWallet();
    const { data: accountsFull } = useQuery({
        queryKey: ['account-list-full'],
        queryFn: async () => {
            try {
                // eslint-disable-next-line @typescript-eslint/ban-ts-comment
                // @ts-ignore
                const e = await currentWallet.features['standard:accountList']?.get();
                return e;
            } catch (error) {
                console.error(
                    'Failed to disconnect the application from the current wallet.',
                    error,
                );
            }
            return [''];
        },
        enabled: !!currentWallet,
    });

    const handleGetAccounts = async () => {
        console.log('--- t');
        try {
        // eslint-disable-next-line @typescript-eslint/ban-ts-comment
        // @ts-ignore
            const e = await currentWallet.features['standard:accountList']?.get();
            console.log('--- ', e);
        } catch (e) {
            console.log('--- err', e);
        }
    };

    return (
        <main className="flex min-h-screen flex-col items-center space-y-8 p-24">
            <div onClick={handleGetAccounts}>get accounts list</div>
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
