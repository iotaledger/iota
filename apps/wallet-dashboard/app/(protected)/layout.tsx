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

            {/* This padding need to have aligned left/right content's position, because of sidebar overlap on the small screens */}
            <div className="pl-[72px]">
                <div className="container relative flex min-h-screen flex-col">
                    <div className="sticky top-0 z-10 backdrop-blur-lg">
                        <TopNav />
                    </div>
                    <div className="flex-1 py-md--rs">{children}</div>
                </div>
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
