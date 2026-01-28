// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMutation, useQueryClient } from '@tanstack/react-query';
import { SIDE_PANEL_QUERY_KEY } from './useSidePanel';
import { SidePanel } from '_src/polyfills/sidepanel';

export function useSidePanelMutation() {
    const queryClient = useQueryClient();
    return useMutation({
        mutationKey: ['set sidepanel mutation'],
        mutationFn: async (enable: boolean) => {
            if (enable) {
                await SidePanel.open('ui.html');
            } else {
                await SidePanel.disable();
                await SidePanel.close();
            }
        },
        onSettled: () => {
            queryClient.invalidateQueries({ exact: true, queryKey: SIDE_PANEL_QUERY_KEY });
        },
    });
}
