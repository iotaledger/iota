// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
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
                <div className="flex flex-col gap-6 overflow-y-auto">
                    {noExpandableList ? <>{items}</> : <ExpandableListItems />}
                </div>

                {items.length > itemsToShow && (
                    <div className="mt-6">
                        <ExpandableListControl />
                    </div>
                )}
            </ExpandableList>
        </CollapsibleCard>
    );
}
