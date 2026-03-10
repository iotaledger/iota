// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { type PropsWithChildren } from 'react';

export default function AuctionLayout({ children }: PropsWithChildren): JSX.Element {
    return (
        <main className="flex flex-col min-h-screen">
            <div className="container w-full h-full pt-20 flex flex-col flex-1">
                <div className="flex flex-col w-full gap-y-lg py-lg flex-1">{children}</div>
            </div>
        </main>
    );
}
