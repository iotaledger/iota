// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode } from 'react';
import {
    CollapsibleCard,
    ExpandableList,
    ExpandableListControl,
    ExpandableListItems,
} from '~/components/ui';

interface ProgrammableTxnBlockCardProps {
    items: ReactNode[];
    itemsLabel: string;
    defaultItemsToShow?: number;
    noExpandableList?: boolean;
    count?: number;
    initialClose?: boolean;
}

export function ProgrammableTxnBlockCard({
    items,
    itemsLabel,
    noExpandableList,
    count,
    initialClose,
    defaultItemsToShow,
}: ProgrammableTxnBlockCardProps): JSX.Element | null {
    if (!items?.length) {
        return null;
    }

    const cardTitle = count ? `${count} ${itemsLabel}` : itemsLabel;
    const itemsToShow = defaultItemsToShow || items.length;

    return (
        <CollapsibleCard collapsible initialClose={initialClose} title={cardTitle}>
            <ExpandableList items={items} defaultItemsToShow={itemsToShow} itemsLabel={itemsLabel}>
                <div className="flex flex-col gap-xs overflow-y-auto p-md--rs pt-xs--rs">
                    {noExpandableList ? <>{items}</> : <ExpandableListItems />}
                </div>

                {items.length > itemsToShow && (
                    <div className="pb-md">
                        <ExpandableListControl />
                    </div>
                )}
            </ExpandableList>
        </CollapsibleCard>
    );
}
