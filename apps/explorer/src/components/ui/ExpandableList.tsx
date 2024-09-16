// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode, useMemo, useState, createContext, useContext } from 'react';
import { Link } from './Link';
import { ArrowRight } from '@iota/ui-icons';

type ExpandableListContextType = {
    handleShowAllClick: () => void;
    showAll: boolean;
    items: ReactNode[];
    defaultItemsToShow: number;
    itemsLabel?: string;
};

const ExpandableListContext = createContext<ExpandableListContextType | null>(null);

export function ExpandableListItems(): JSX.Element {
    const listContext = useContext(ExpandableListContext);

    if (!listContext) {
        throw new Error('ExpandableListItems must be used within an ExpandableList');
    }

    const { showAll, items, defaultItemsToShow } = listContext;

    const itemsDisplayed = useMemo(
        () => (showAll ? items : items?.slice(0, defaultItemsToShow)),
        [showAll, items, defaultItemsToShow],
    );

    return <>{itemsDisplayed}</>;
}

export function ExpandableListControl(): JSX.Element | null {
    const listContext = useContext(ExpandableListContext);

    if (!listContext) {
        throw new Error('ExpandableListControl must be used within an ExpandableList');
    }

    const { handleShowAllClick, showAll, items, itemsLabel, defaultItemsToShow } = listContext;

    let showAllText = '';
    if (showAll) {
        showAllText = 'Show Less';
    } else {
        showAllText = itemsLabel ? `Show All ${items.length} ${itemsLabel}` : 'Show All';
    }

    if (items.length <= defaultItemsToShow) {
        return null;
    }

    return (
        <div className="flex cursor-pointer items-center text-neutral-40 dark:text-neutral-60">
            <Link variant="text" onClick={handleShowAllClick}>
                <div className="flex items-center gap-xxxs">
                    <span className="text-body-sm ">{showAllText}</span>
                    <ArrowRight />
                </div>
            </Link>
        </div>
    );
}

interface ExpandableListProps {
    items: ReactNode[];
    defaultItemsToShow: number;
    itemsLabel?: string;
    children?: ReactNode;
}

export function ExpandableList({
    items,
    defaultItemsToShow,
    itemsLabel,
    children,
}: ExpandableListProps): JSX.Element {
    const [showAll, setShowAll] = useState(false);

    const handleShowAllClick = () => setShowAll((prevShowAll: boolean) => !prevShowAll);

    return (
        <ExpandableListContext.Provider
            value={{
                handleShowAllClick,
                showAll,
                items,
                defaultItemsToShow,
            }}
        >
            {children || (
                <>
                    <ExpandableListItems />
                    <ExpandableListControl />
                </>
            )}
        </ExpandableListContext.Provider>
    );
}
