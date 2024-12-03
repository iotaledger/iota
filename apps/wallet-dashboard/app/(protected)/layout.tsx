// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Notifications } from '@/components/index';
import React, { type PropsWithChildren } from 'react';
import { Button } from '@iota/apps-ui-kit';
import { Sidebar, TopNav } from './components';
import { ThemePreference, useTheme } from '@iota/core';

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
    const { theme, themePreference, setThemePreference } = useTheme();

    const toggleTheme = () => {
        const newTheme =
            themePreference === ThemePreference.Light
                ? ThemePreference.Dark
                : ThemePreference.Light;
        setThemePreference(newTheme);
    };

    return (
        <div className="min-h-full">
            <div className="fixed left-0 top-0 z-50 h-full">
                <Sidebar />
            </div>

            <div className="container relative flex min-h-screen flex-col">
                <div className="sticky top-0 z-10 bg-gradient-to-b from-gray-100 via-gray-100 to-gray-100/8 dark:from-gray-900 dark:via-gray-900 dark:to-gray-900/8">
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
    );
}

export default DashboardLayout;
