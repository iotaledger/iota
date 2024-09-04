// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type ReactNode,
    useContext,
    createContext,
    useState,
    type Dispatch,
    type SetStateAction,
} from 'react';

type TabsContextProps = {
    activeTabId: string;
    setActiveTabId: Dispatch<SetStateAction<string>>;
} | null;

const TabsContext = createContext<TabsContextProps>(null);

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

export function TabsTrigger({ tabId, children }: { tabId: string; children?: ReactNode }) {
    const context = useContext<TabsContextProps>(TabsContext);

    return (
        <div className="flex" onClick={() => context?.setActiveTabId(tabId)}>
            {children}
        </div>
    );
}

export function TabsHeader({
    children,
    render,
}: {
    children?: ReactNode;
    render?: (tabId: string) => ReactNode;
}) {
    const context = useContext<TabsContextProps>(TabsContext);
    if (!context || !context.activeTabId) return null;
    return <div>{render ? render(context.activeTabId) : children}</div>;
}

export function TabsContent({ tabId, children }: { children: ReactNode; tabId: string }) {
    const context = useContext<TabsContextProps>(TabsContext);
    if (!context || !context.activeTabId || tabId !== context.activeTabId) return null;
    return <div className="px-lg py-md">{children}</div>;
}
