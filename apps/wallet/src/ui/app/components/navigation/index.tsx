// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useActiveAccount } from '../../hooks/useActiveAccount';
import { Navbar, type NavbarItemWithId } from '@iota/apps-ui-kit';
import { Activity, Assets, Home } from '@iota/ui-icons';

export function Navigation() {
    const activeAccount = useActiveAccount();

    const navigate = useNavigate();
    const onHomeClick = () => {
        navigate('/tokens');
    };

    const onAssetsClick = () => {
        navigate('/nfts');
    };

    const onActivityClick = () => {
        navigate('/transactions');
    };

    const NAVBAR_ITEMS: NavbarItemWithId[] = [
        { id: 'home', icon: <Home />, onClick: onHomeClick },
        {
            id: 'assets',
            icon: <Assets />,
            onClick: onAssetsClick,
            isDisabled: activeAccount?.isLocked,
        },
        {
            id: 'activity',
            icon: <Activity />,
            onClick: onActivityClick,
            isDisabled: activeAccount?.isLocked,
        },
    ];
    const [activeRouteId, setActiveRouteId] = useState<string>(NAVBAR_ITEMS[0].id);
    return (
        <div className="sticky bottom-0 w-full shrink-0 rounded-tl-md rounded-tr-md border-b-0 bg-white">
            <Navbar
                items={NAVBAR_ITEMS}
                activeId={activeRouteId}
                onClickItem={(id) => setActiveRouteId(id)}
                areSomeItemsDisabled={activeAccount?.isLocked}
            />
        </div>
    );
}
