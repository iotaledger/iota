// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { createContext, Dispatch, SetStateAction } from 'react';

export type TabsContextProps = {
    activeTabId: string;
    setActiveTabId: Dispatch<SetStateAction<string>>;
} | null;

export const TabsContext = createContext<TabsContextProps>(null);
