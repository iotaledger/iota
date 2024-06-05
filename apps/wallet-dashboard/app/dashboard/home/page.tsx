// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { AccountBalance, AllCoins } from '@/components';

function HomeDashboardPage(): JSX.Element {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();

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
