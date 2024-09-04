// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ReactNode, useContext } from 'react';
import { TabsContext, TabsContextProps } from '@/components/atoms/tabs/TabsContext';

export function TabsContent({ tabId, children }: { children: ReactNode; tabId: string }) {
    const context = useContext<TabsContextProps>(TabsContext);
    if (!context || !context.activeTabId || tabId !== context.activeTabId) return null;
    return <div className="px-lg py-md">{children}</div>;
}
