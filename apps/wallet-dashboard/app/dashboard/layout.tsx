// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Notifications, RouteLink } from '@/components/index';
import React, { type PropsWithChildren } from 'react';
import { ConnectButton } from '@iota/dapp-kit';

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
    const routes = [
        { title: 'Home', path: '/dashboard/home' },
        { title: 'Assets', path: '/dashboard/assets' },
        { title: 'Staking', path: '/dashboard/staking' },
        { title: 'Apps', path: '/dashboard/apps' },
        { title: 'Activity', path: '/dashboard/activity' },
        { title: 'Migrations', path: '/dashboard/migrations' },
        { title: 'Vesting', path: '/dashboard/vesting' },
    ];

    // TODO: check if the wallet is connected and if not redirect to the welcome screen
    return (
        <>
            <section className="flex flex-row items-center justify-around pt-12">
                <Notifications />
                <ConnectButton />

                {routes.map((route) => {
                    return <RouteLink key={route.title} {...route} />;
                })}
            </section>
            <div>{children}</div>
        </>
    );
}

export default DashboardLayout;
