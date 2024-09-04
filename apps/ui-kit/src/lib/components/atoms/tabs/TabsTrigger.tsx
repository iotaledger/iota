// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ReactNode, useContext } from 'react';
import { TabsContext, TabsContextProps } from './TabsContext';

export function TabsTrigger({ tabId, children }: { tabId: string; children?: ReactNode }) {
    const context = useContext<TabsContextProps>(TabsContext);

    return (
        <div className="flex" onClick={() => context?.setActiveTabId(tabId)}>
            {children}
        </div>
    );
}
