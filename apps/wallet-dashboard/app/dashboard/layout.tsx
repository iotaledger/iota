// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Notifications, RouteLink } from '@/components/index';
import React, { useEffect, useState, type PropsWithChildren } from 'react';
import { ConnectButton, useCurrentAccount, useCurrentWallet } from '@iota/dapp-kit';
import { Button } from '@iota/apps-ui-kit';
import { useRouter } from 'next/navigation';
import { useFeature } from '@growthbook/growthbook-react';
import { Feature } from '@iota/core';

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
    const { connectionStatus } = useCurrentWallet();
    const account = useCurrentAccount();
    const router = useRouter();

    const featureFlags = {
        Migrations: useFeature<boolean>(Feature.WalletDashboardMigration).value,
        Vesting: useFeature<boolean>(Feature.WalletDashboardSupplyIncreaseVesting).value,
    };

    const filteredRoutes = routes.filter(({ title }) => {
        return title in featureFlags ? featureFlags[title as keyof typeof featureFlags] : true;
    });

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
            router.push('/');
        }
    }, [connectionStatus, account, router]);

    return (
        <>
            <section className="flex flex-row items-center justify-around pt-12">
                <Notifications />
                {filteredRoutes.map((route) => {
                    return <RouteLink key={route.title} {...route} />;
                })}
                <Button onClick={toggleDarkMode} text={isDarkMode ? 'Light Mode' : 'Dark Mode'} />
                <ConnectButton />
            </section>
            <div>{children}</div>
        </>
    );
}

export default DashboardLayout;
