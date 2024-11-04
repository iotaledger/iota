// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Notifications } from '@/components/index';
import React, { useEffect, type PropsWithChildren } from 'react';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { Button } from '@iota/apps-ui-kit';
import { redirect } from 'next/navigation';
import { Sidebar } from './components';
import { TopNav } from './components/top-nav/TopNav';
import { useTheme } from '@/contexts';

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
    const { connectionStatus } = useCurrentWallet();
    const { theme, toggleTheme } = useTheme();
    const account = useCurrentAccount();
    useEffect(() => {
        if (connectionStatus !== 'connected' && !account) {
            redirect('/');
        }
    }, [connectionStatus, account]);

    return (
        <div className="h-full">
            <div className="fixed left-0 top-0 z-50 h-full">
                <Sidebar />
            </div>

            <div className="container relative min-h-screen">
                <div className="sticky top-0">
                    <TopNav />
                </div>
                <div>{children}</div>
            </div>

            <div className="fixed bottom-5 right-5">
                <Button
                    onClick={toggleTheme}
                    text={`${theme === 'dark' ? 'Light' : 'Dark'} mode`}
                />
            </div>

            <Notifications />
        </div>
    );
}

export default DashboardLayout;
