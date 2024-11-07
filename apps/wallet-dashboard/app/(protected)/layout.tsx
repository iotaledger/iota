// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Notifications } from '@/components/index';
import React, { type PropsWithChildren } from 'react';
import { Button } from '@iota/apps-ui-kit';
import { Sidebar } from './components';
import { TopNav } from './components/top-nav/TopNav';
import { useTheme } from '@/contexts';
import { ConnectionGuard } from './components/connection-guard';

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
    const { theme, toggleTheme } = useTheme();

    return (
        <ConnectionGuard>
            <div className="h-full">
                <div className="fixed left-0 top-0 z-50 h-full">
                    <Sidebar />
                </div>

                <div className="container relative flex min-h-screen flex-col">
                    <div className="sticky top-0">
                        <TopNav />
                    </div>
                    <div className="flex-1 py-md--rs">{children}</div>
                </div>

                <div className="fixed bottom-5 right-5">
                    <Button
                        onClick={toggleTheme}
                        text={`${theme === 'dark' ? 'Light' : 'Dark'} mode`}
                    />
                </div>

                <Notifications />
            </div>
        </ConnectionGuard>
    );
}

export default DashboardLayout;
