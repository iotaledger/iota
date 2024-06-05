// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung 
// SPDX-License-Identifier: Apache-2.0

import { useNextMenuUrl } from '_components/menu/hooks';
import NetworkSelector from '_components/network-selector';

import { MenuLayout } from './MenuLayout';

export function NetworkSettings() {
    const mainMenuUrl = useNextMenuUrl(true, '/');
    return (
        <MenuLayout title="Network" back={mainMenuUrl}>
            <NetworkSelector />
        </MenuLayout>
    );
}
