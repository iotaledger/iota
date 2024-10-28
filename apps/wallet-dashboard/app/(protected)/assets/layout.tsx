// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { RouteLink } from '@/components/index';
import { ASSETS_ROUTE } from '@/lib/constants/routes.constants';
import React, { type PropsWithChildren } from 'react';

function AssetsLayout({ children }: PropsWithChildren): JSX.Element {
    const routes = [
        { title: 'Visual Assets', path: ASSETS_ROUTE.path + '/visual-assets' },
        { title: 'Everything Else', path: ASSETS_ROUTE.path + '/everything-else' },
    ];

    return (
        <>
            <section className="flex flex-row items-center justify-center gap-16 p-20 text-blue-400">
                {routes.map((route) => {
                    return <RouteLink key={route.title} {...route} />;
                })}
            </section>
            <div>{children}</div>
        </>
    );
}

export default AssetsLayout;
