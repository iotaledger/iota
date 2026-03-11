// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useLocation } from 'react-router-dom';
import { Navbar, type NavbarItemWithId } from '@iota/apps-ui-kit';
import { Activity, Apps, Assets, Home } from '@iota/apps-ui-icons';
import { useAppSelector } from '_hooks';
import { ExtensionViewType } from '_src/ui/app/redux/slices/app/appType';

type NavbarItemWithPath = NavbarItemWithId & {
    path: string;
};

export function Navigation() {
    const navigate = useNavigate();
    const location = useLocation();
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
    const isFullScreen = extensionViewType === ExtensionViewType.FullScreen;
    const isPopup = extensionViewType === ExtensionViewType.Popup;
    const useSidebar = isFullScreen || isPopup;

    const NAVBAR_ITEMS: NavbarItemWithPath[] = [
        { id: 'home', icon: <Home />, path: '/tokens' },
        {
            id: 'assets',
            icon: <Assets />,
            path: '/nfts',
        },
        {
            id: 'apps',
            icon: <Apps />,
            path: '/apps',
        },
        {
            id: 'activity',
            icon: <Activity />,
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

    if (useSidebar) {
        return (
            <div className="flex h-full w-16 shrink-0 flex-col items-center gap-xs border-r border-shader-neutral-light-8 bg-iota-neutral-100 dark:border-shader-neutral-dark-8 dark:bg-iota-neutral-6">
                {NAVBAR_ITEMS.map((item) => (
                    <div
                        key={item.id}
                        className="flex items-center px-xs py-xxs"
                        data-testid={`nav-${item.id}`}
                    >
                        <Navbar items={[item]} activeId={activeId} onClickItem={handleItemClick} />
                    </div>
                ))}
            </div>
        );
    }

    return (
        <div className="sticky bottom-0 w-full shrink-0 border-b-0 bg-iota-neutral-100 dark:bg-iota-neutral-6">
            <Navbar items={NAVBAR_ITEMS} activeId={activeId} onClickItem={handleItemClick} />
        </div>
    );
}
