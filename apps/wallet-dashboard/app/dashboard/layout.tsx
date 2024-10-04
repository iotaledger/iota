// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Button, Notifications, RouteLink } from '@/components/index';
import React, { useState, type PropsWithChildren } from 'react';
import { ConnectButton } from '@iota/dapp-kit';

const routes = [
    { title: 'Home', path: '/dashboard/home' },
    { title: 'Assets', path: '/dashboard/assets' },
    { title: 'Staking', path: '/dashboard/staking' },
    { title: 'Apps', path: '/dashboard/apps' },
    { title: 'Activity', path: '/dashboard/activity' },
    { title: 'Migrations', path: '/dashboard/migrations' },
    { title: 'Vesting', path: '/dashboard/vesting' },
];

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
    const [isDarkMode, setIsDarkMode] = useState(false);

    const toggleDarkMode = () => {
        setIsDarkMode(!isDarkMode);
        if (isDarkMode) {
            document.documentElement.classList.remove('dark');
        } else {
            document.documentElement.classList.add('dark');
        }
    };

    // TODO: check if the wallet is connected and if not redirect to the welcome screen
    return (
        <>
            <section className="flex flex-row items-center justify-around pt-12">
                <Notifications />
                {routes.map((route) => {
                    return <RouteLink key={route.title} {...route} />;
                })}
                <Button onClick={toggleDarkMode}>{isDarkMode ? 'Light Mode' : 'Dark Mode'}</Button>
                <ConnectButton />
            </section>
            <div>{children}</div>
        </>
    );
}

export default DashboardLayout;
