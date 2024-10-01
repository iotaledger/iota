// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useState } from 'react';
import type { TabItem } from './tabs.interfaces';

interface TabContextProps {
    tabs: TabItem[];
    selectedTabId: string;
    setSelectedTabId: (id: string) => void;
}

const TabsContext = createContext<TabContextProps>({
    tabs: [],
    selectedTabId: '',
    setSelectedTabId: () => {},
});

const useTabsContext = (): TabContextProps => {
    const context = useContext(TabsContext);
    if (!context) {
        throw new Error('useTabsContext must be used within a TabsProvider');
    }
    return context;
};

interface TabbedContentProps {
    tabs: TabItem[];
    initialSelectedTabId?: string;
}

export function TabsProvider({
    children,
    initialSelectedTabId,
    tabs,
}: React.PropsWithChildren<TabbedContentProps>): React.ReactNode {
    initialSelectedTabId ??= tabs.length > 0 ? tabs[0].id : '';
    const [selectedTabId, setSelectedTabId] = useState<string>(initialSelectedTabId);

    return (
        <TabsContext.Provider value={{ tabs, selectedTabId, setSelectedTabId }}>
            {children}
        </TabsContext.Provider>
    );
}

interface TabSelectorProps {
    children: (selectedTabId: string, setSelectedTabId: (id: string) => void) => React.ReactNode;
}

export function TabSelector({ children }: TabSelectorProps): React.ReactNode {
    const { selectedTabId, setSelectedTabId } = useTabsContext();
    return children(selectedTabId, setSelectedTabId);
}

interface TabContentProps {
    id: string;
}

export function TabContent({
    children,
    id,
}: React.PropsWithChildren<TabContentProps>): React.ReactNode {
    const { selectedTabId } = useTabsContext();
    return selectedTabId === id ? children : null;
}
