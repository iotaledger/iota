// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { IotaLogoMark, MenuIcon } from '@iota/apps-ui-icons';
import { NavbarItem, NavbarItemProps } from '@/components/molecules/navbar-item/NavbarItem';

export type NavbarItemWithId = NavbarItemProps & { id: string };

export interface NavbarProps {
    /**
     * If this flag is true we need to leave only the icon and collapsible button
     */
    isCollapsible?: boolean;

    /**
     * List of elements to be displayed in the navbar.
     */
    items: NavbarItemWithId[];

    /**
     * The id of the active element.
     */
    activeId: string;

    /**
     * Callback when an element is clicked.
     */
    onClickItem: (id: string) => void;

    /**
     * If the navbar is collapsible, this flag indicates if it is open or not.
     */
    isOpen?: boolean;

    /**
     * Callback when the navbar is toggled.
     */
    onToggleNavbar?: () => void;
}

export function Navbar({
    items,
    activeId,
    onClickItem,
    isCollapsible = false,
    onToggleNavbar,
}: NavbarProps) {
    return (
        <div
            className={cx('flex h-fit w-full', {
                'flex-col px-md py-xs sm:h-full sm:w-auto sm:px-none sm:py-xl': isCollapsible,
            })}
        >
            {isCollapsible && (
                <div className="flex w-full items-center justify-between sm:mb-[48px] sm:flex-col">
                    <div className="flex justify-center [&_svg]:h-[38px] [&_svg]:w-[38px]">
                        <IotaLogoMark className="text-neutral-10 dark:text-neutral-92" />
                    </div>
                    <div
                        className="state-layer relative rounded-full p-xs hover:cursor-pointer dark:text-neutral-92 sm:hidden [&_svg]:h-6 [&_svg]:w-6"
                        onClick={onToggleNavbar}
                    >
                        <MenuIcon />
                    </div>
                </div>
            )}
            <div
                className={cx({
                    'flex w-full justify-between px-sm py-xxs': !isCollapsible,
                    'hidden sm:flex sm:flex-col sm:gap-2': isCollapsible,
                })}
            >
                {items.map((item) => (
                    <div
                        key={item.id}
                        className={cx('flex items-center', {
                            'px-xs py-xxs': !isCollapsible,
                            'py-xxs pl-xs pr-sm': isCollapsible,
                        })}
                        data-testid={`nav-${item.id}`}
                    >
                        <NavbarItem
                            {...item}
                            isSelected={item.id === activeId}
                            onClick={(e) => {
                                if (item.onClick) {
                                    item.onClick(e);
                                }
                                if (!item.isDisabled) {
                                    onClickItem(item.id);
                                }
                            }}
                        />
                    </div>
                ))}
            </div>
        </div>
    );
}
