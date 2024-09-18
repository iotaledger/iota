// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TabContent } from '../tabs';

export function TabbedContentWrapper({ children }: React.PropsWithChildren): React.ReactNode {
    return <div className="flex h-full flex-col md:pl-lg">{children}</div>;
}

interface ListTabContentProps {
    id: string;
}

export function ListTabContent({
    children,
    id,
}: React.PropsWithChildren<ListTabContentProps>): React.ReactNode {
    return children ? (
        <TabContent id={id}>
            <div className="overflow-auto pt-sm--rs">{children}</div>
        </TabContent>
    ) : null;
}
