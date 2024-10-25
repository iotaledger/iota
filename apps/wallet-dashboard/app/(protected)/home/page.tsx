// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { AccountBalance, MyCoins, Button, NewStakePopup } from '@/components';
import { usePopups } from '@/hooks';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';

function HomeDashboardPage(): JSX.Element {
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();
    const { openPopup, closePopup } = usePopups();

    const addNewStake = () => {
        openPopup(<NewStakePopup onClose={closePopup} />);
    };

    return (
        <main className="flex min-h-screen flex-col items-center space-y-8 p-24">
            <p>Connection status: {connectionStatus}</p>
            {connectionStatus === 'connected' && account && (
                <div className="flex flex-col items-center justify-center space-y-2">
                    <h1>Welcome</h1>
                    <div>Address: {account.address}</div>
                    <AccountBalance />
                    <MyCoins />
                    <Button onClick={addNewStake}>New Stake</Button>
                </div>
            )}
        </main>
    );
}

export default HomeDashboardPage;
