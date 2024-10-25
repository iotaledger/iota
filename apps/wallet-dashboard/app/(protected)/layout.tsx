// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Notifications, RouteLink } from '@/components/index';
import React, { useEffect, useState, type PropsWithChildren } from 'react';
import { ConnectButton, useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { Button } from '@iota/apps-ui-kit';
import { redirect } from 'next/navigation';
import { PROTECTED_ROUTES } from '@/lib/constants';
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
            <Sidebar />
            <div className="container">
                <TopNav />
                <div className="flex flex-row items-center justify-around pt-12">
                    <Notifications />
                    {PROTECTED_ROUTES.map(({ title, path }) => (
                        <RouteLink key={title} title={title} path={path} />
                    ))}
                    <Button
                        onClick={toggleDarkMode}
                        text={isDarkMode ? 'Light Mode' : 'Dark Mode'}
                    />
                    <ConnectButton />
                </div>
            </div>
            <div>{children}</div>
        </div>
    );
}

export default DashboardLayout;
