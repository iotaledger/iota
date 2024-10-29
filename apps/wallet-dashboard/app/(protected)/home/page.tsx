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
        <main className="flex flex-1 flex-col items-center space-y-8 py-md">
            {connectionStatus === 'connected' && account && (
                <>
                    <div className="home-page-grid-container h-full w-full">
                        <div style={{ gridArea: 'balance' }} className="flex grow overflow-hidden">
                            <AccountBalance />
                        </div>
                        <div style={{ gridArea: 'staking' }} className="flex grow overflow-hidden">
                            Staking
                        </div>
                        <div
                            style={{ gridArea: 'migration' }}
                            className="flex grow overflow-hidden"
                        >
                            Migration
                        </div>
                        <div style={{ gridArea: 'coins' }}>
                            <MyCoins />
                        </div>
                        <div style={{ gridArea: 'vesting' }} className="flex grow overflow-hidden">
                            Vesting
                        </div>
                        <div style={{ gridArea: 'activity' }} className="flex grow overflow-hidden">
                            Activity
                        </div>
                    </div>
                    <Button onClick={addNewStake}>New Stake</Button>
                </>
            )}
        </main>
    );
}

export default HomeDashboardPage;
