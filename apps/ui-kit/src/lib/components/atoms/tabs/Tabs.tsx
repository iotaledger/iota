// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useState } from 'react';
import { TabsContext } from './TabsContext';

export function Tabs({ children, defaultTabId }: { children: ReactNode; defaultTabId: string }) {
    const [activeTabId, setActiveTabId] = useState(defaultTabId);
    return (
        <TabsContext.Provider
            value={{
                activeTabId,
                setActiveTabId,
            }}
        >
            {children}
        </TabsContext.Provider>
    );
}
