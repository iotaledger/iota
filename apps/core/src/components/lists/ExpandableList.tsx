// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import { useMemo, useState, type ReactNode } from 'react';
import { TriangleDown } from '@iota/ui-icons';
import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';

interface ExpandableListProps {
    items: ReactNode[];
    defaultItemsToShow: number;
    initialShowAll?: boolean;
}

export function ExpandableList({ items, defaultItemsToShow, initialShowAll }: ExpandableListProps) {
    const [showAll, setShowAll] = useState(initialShowAll || false);

    const itemsDisplayed = useMemo(
        () => (showAll ? items : items?.slice(0, defaultItemsToShow)),
        [showAll, items, defaultItemsToShow],
    );

    const handleShowAllClick = () => setShowAll((prevShowAll: boolean) => !prevShowAll);

    return (
        <>
            {itemsDisplayed.map((item, index) => (
                <div key={index}>{item}</div>
            ))}
            {items.length > defaultItemsToShow && (
                <div className="flex w-full cursor-pointer items-center justify-center">
                    <Button
                        size={ButtonSize.Small}
                        type={ButtonType.Ghost}
                        onClick={(e) => {
                            e.stopPropagation();
                            handleShowAllClick();
                        }}
                        text={showAll ? 'Show Less' : 'Show All'}
                        iconAfterText
                        icon={
                            <TriangleDown
                                className={clsx(
                                    'ml-xxxs h-5 w-5 text-neutral-60',
                                    showAll
                                        ? 'rotate-180 transition-transform ease-linear'
                                        : 'rotate-0 transition-transform ease-linear',
                                )}
                            />
                        }
                    />
                </div>
            )}
        </>
    );
}
