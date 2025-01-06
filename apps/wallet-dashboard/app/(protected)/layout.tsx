// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import React, { type PropsWithChildren } from 'react';
import { Sidebar, TopNav } from './components';

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
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
        </div>
    );
}

export default DashboardLayout;
