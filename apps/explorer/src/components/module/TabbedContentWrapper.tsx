// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import { TabContent } from '../tabs';

export function TabbedContentWrapper({ children }: React.PropsWithChildren): React.ReactNode {
    return <div className="h-full grow overflow-auto border-gray-45 md:pl-7">{children}</div>;
}

interface ListTabContentProps {
    isCompact: boolean;
    id: string;
}

export function ListTabContent({
    children,
    isCompact,
    id,
}: React.PropsWithChildren<ListTabContentProps>): React.ReactNode {
    return (
        <TabContent id={id}>
            <div className={clsx('overflow-auto pt-sm--rs', { 'h-verticalListLong': isCompact })}>
                {children}
            </div>
        </TabContent>
    );
}
