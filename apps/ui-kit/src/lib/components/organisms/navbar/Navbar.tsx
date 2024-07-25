// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { NavbarItem, NavbarItemProps } from '@/components/molecules/navbar-item/NavbarItem';

export type NavbarItemWithID = NavbarItemProps & { id: string };

export interface NavbarProps {
    /**
     * List of elements to be displayed in the navbar.
     */
    items: NavbarItemWithID[];

    /**
     * The id of the active element.
     */
    activeId: string;

    /**
     * Callback when an element is clicked.
     * @param id
     */
    onClick: (id: string) => void;

    /**
     * Additional classes for the navbar if needs.
     */
    className?: string;
}

function Navbar({ items, activeId, onClick, className }: NavbarProps) {
    return (
        <div className={cx('flex w-full justify-between gap-2', className)}>
            {items.map((item) => (
                <div key={item.id} className="px-xs py-xxs">
                    <NavbarItem
                        {...item}
                        isSelected={item.id === activeId}
                        onClick={() => onClick(item.id)}
                    />
                </div>
            ))}
        </div>
    );
}

export { Navbar };
