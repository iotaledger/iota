// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client'

import { usePathname } from 'next/navigation';
import React, { type PropsWithChildren } from 'react';
import Link from 'next/link'

function DashboardLayout({ children }: PropsWithChildren): JSX.Element {
	const path = usePathname();
    const DASHBOARD_HOME_ROUTE = '/dashboard/home'

    const isActive = (pathname: string) => path && (pathname === path || pathname.startsWith(path))

	const routes: { title: string; path: string }[] = [
		{ title: 'Home', path: DASHBOARD_HOME_ROUTE },
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
                        <Link
                            href={route.path ?? ''}
                            key={route.title}
                        >
                            <div
                                className={`sidebar-item justify-between space-x-5 ${isActive(route.path) ? 'underline' : ''}`}
                            >
                                <div className="flex items-center justify-between space-x-5">
                                    <span className="origin-left duration-300 flex-shrink-0">{route.title}</span>
                                </div>
                            </div>
                        </Link>
                    )
                })}
			</section>
            <div>{children}</div>
		</>
	);
}

export default DashboardLayout;
