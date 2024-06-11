// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode } from 'react';

type FooterItem = {
    category: string;
    items: { title: string; children: ReactNode; href: string }[];
};
export type FooterItems = FooterItem[];

export function FooterIcon({ children }: { children: ReactNode }) {
    return <div className="flex items-center text-steel-darker">{children}</div>;
}
