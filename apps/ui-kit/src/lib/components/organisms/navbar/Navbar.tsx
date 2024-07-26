// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { IotaLogoMark, Menu } from '@iota/ui-icons';
import { NavbarItem, NavbarItemProps } from '@/components/molecules/navbar-item/NavbarItem';

export type NavbarItemWithID = NavbarItemProps & { id: string };

export interface NavbarProps {
    /**
     * If this flag is true we need to leave only the icon and collapsable button
     */
    isCollapsable?: boolean;

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
}

export function Navbar({ items, activeId, onClick, isCollapsable = false }: NavbarProps) {
    const handleMenuClick = () => {
        console.info('Menu clicked');
    };

    return (
        <div
            className={cx({
                'flex w-full': !isCollapsable,
                'w-full flex-col justify-between gap-2 px-sm py-xs sm:w-auto sm:py-xl':
                    isCollapsable,
            })}
        >
            {isCollapsable && (
                <div className="flex w-full justify-between sm:mb-[48px] sm:flex-col">
                    <div className="flex justify-center">
                        <IotaLogoMark width={38} height={38} />
                    </div>
                    <div
                        className="state-layer relative rounded-full p-xs hover:cursor-pointer sm:hidden"
                        onClick={handleMenuClick}
                    >
                        <Menu width={24} height={24} />
                    </div>
                </div>
            )}
            <div
                className={cx({
                    'flex w-full justify-between': !isCollapsable,
                    'hidden sm:flex sm:flex-col': isCollapsable,
                })}
            >
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
        </div>
    );
}
