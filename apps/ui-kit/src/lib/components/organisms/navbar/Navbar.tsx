// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { IotaLogoMark, Menu } from '@iota/ui-icons';
import { NavbarItem, NavbarItemProps } from '@/components/molecules/navbar-item/NavbarItem';
import { NavbarType } from '@/components/organisms/navbar/navbar.enums';

export type NavbarItemWithID = NavbarItemProps & { id: string };

export interface NavbarProps {
    type?: NavbarType;

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

const NAVBAR_CLASSES = {
    [NavbarType.Horizontal]: 'flex w-full justify-between gap-2',
    [NavbarType.Vertical]: 'flex-col justify-between gap-2 w-full py-xs px-sm sm:py-xl sm:w-[80px]',
} as const;

function Navbar({
    items,
    activeId,
    onClick,
    className,
    type = NavbarType.Horizontal,
}: NavbarProps) {
    const handleMenuClick = () => {
        console.info('Menu clicked');
    };

    return (
        <div className={cx(className, NAVBAR_CLASSES[type])}>
            {type === NavbarType.Vertical && (
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
            <div className={cx('hidden sm:flex', { 'flex-col': type === NavbarType.Vertical })}>
                {items.map((item) => (
                    <div key={item.id} className=" px-xs py-xxs  ">
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

export { Navbar };
