// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ReactNode, useContext } from 'react';
import { TabsContext, TabsContextProps } from './TabsContext';

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
