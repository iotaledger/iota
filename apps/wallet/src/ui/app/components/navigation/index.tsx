// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useLocation } from 'react-router-dom';
import { useActiveAccount } from '_hooks';
import { Navbar, type NavbarItemWithId } from '@iota/apps-ui-kit';
import { Activity, Apps, Assets, Home } from '@iota/apps-ui-icons';

type NavbarItemWithPath = NavbarItemWithId & {
    path: string;
};

export function Navigation() {
    const activeAccount = useActiveAccount();
    const navigate = useNavigate();
    const location = useLocation();

    const NAVBAR_ITEMS: NavbarItemWithPath[] = [
        { id: 'home', icon: <Home />, path: '/tokens' },
        {
            id: 'assets',
            icon: <Assets />,
            isDisabled: activeAccount?.isLocked,
            path: '/nfts',
        },
        {
            id: 'apps',
            icon: <Apps />,
            isDisabled: activeAccount?.isLocked,
            path: '/apps',
        },
        {
            id: 'activity',
            icon: <Activity />,
            isDisabled: activeAccount?.isLocked,
            path: '/transactions',
        },
    ];

    const activeId = NAVBAR_ITEMS.find((item) => location.pathname.startsWith(item.path))?.id || '';

    function handleItemClick(id: string) {
        const item = NAVBAR_ITEMS.find((item) => item.id === id);
        if (item && !item.isDisabled) {
            navigate(item.path);
        }
    }

    return (
        <div className="sticky bottom-0 w-full shrink-0 border-b-0 bg-neutral-100 dark:bg-neutral-6">
            <Navbar items={NAVBAR_ITEMS} activeId={activeId} onClickItem={handleItemClick} />
        </div>
    );
}
