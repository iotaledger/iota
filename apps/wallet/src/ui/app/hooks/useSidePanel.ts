// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useQuery } from '@tanstack/react-query';
import { SidePanel } from '_src/polyfills/sidepanel';

export const SIDE_PANEL_QUERY_KEY = ['get sidepanel'];

export function useSidePanel() {
    return useQuery({
        queryKey: SIDE_PANEL_QUERY_KEY,
        queryFn: async () => SidePanel.isEnabled(),
        refetchInterval: 15 * 1000,
        meta: {
            skipPersistedCache: true,
        },
    });
}
