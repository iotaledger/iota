// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Notifications } from '@/components/index';
import React, { useEffect, useState, type PropsWithChildren } from 'react';
import { useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { Button } from '@iota/apps-ui-kit';
import { redirect } from 'next/navigation';
import { Sidebar } from './components';
import { TopNav } from './components/top-nav/TopNav';

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
    const [isDarkMode, setIsDarkMode] = useState(false);
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();

    const toggleDarkMode = () => {
        setIsDarkMode(!isDarkMode);
        if (isDarkMode) {
            document.documentElement.classList.remove('dark');
        } else {
            document.documentElement.classList.add('dark');
        }
    };

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
                <Button onClick={toggleDarkMode} text={isDarkMode ? 'Light Mode' : 'Dark Mode'} />
            </div>

            <Notifications />
        </div>
    );
}

export default DashboardLayout;
