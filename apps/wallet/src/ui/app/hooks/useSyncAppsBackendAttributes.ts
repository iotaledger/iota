// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAppSelector } from '_hooks';
import { setAttributes } from '_src/shared/experimentation/features';
import { useAppsBackendClient } from '@iota/apps-backend-client';
import { useEffect } from 'react';

export function useSyncAppsBackendAttributes() {
    const { network, customRpc } = useAppSelector((state) => state.app);
    const client = useAppsBackendClient();

    useEffect(() => {
        if (client) {
            setAttributes({ network, customRpc });
        }
    }, [client, network, customRpc]);
}
