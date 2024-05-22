// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { type PropsWithChildren } from 'react';
import { RouteLink } from '../components';
import type { IRouteLink } from '../lib/interfaces';

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
	const routes: { title: string; path: string }[] = [
		{ title: 'Home', path: '/dashboard/home' },
		{ title: 'Assets', path: '/dashboard/assets' },
		{ title: 'Staking', path: '/dashboard/staking' },
		{ title: 'Apps', path: '/dashboard/apps' },
		{ title: 'Activity', path: '/dashboard/activity' },
		{ title: 'Migrations', path: '/dashboard/migrations' },
	];

    // TODO: check if the wallet is connected and if not redirect to the welcome screen
	return (
		<>
			<section className="flex flex-row items-center justify-around mt-12">
                {routes.map((route) => {
                    return (
                        <RouteLink key={route.title} {...route} />
                    )
                })}
			</section>
            <div>{children}</div>
		</>
	);
}

export default DashboardLayout;
