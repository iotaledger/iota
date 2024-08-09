// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { IotaLogoMark, MenuIcon } from '@iota/ui-icons';
import { NavbarItem, NavbarItemProps } from '@/components/molecules/navbar-item/NavbarItem';

export type NavbarItemWithId = NavbarItemProps & { id: string };

export interface NavbarProps {
    /**
     * If this flag is true we need to leave only the icon and collapsable button
     */
    isCollapsable?: boolean;

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
     * If the navbar is collapsable, this flag indicates if it is open or not.
     */
    isOpen?: boolean;

    /**
     * Callback when the navbar is toggled.
     */
    onToggleNavbar?: () => void;
    /**
     * Indicates if some items are disabled.
     */
    areSomeItemsDisabled?: boolean;
}

export function Navbar({
    items,
    activeId,
    onClickItem,
    isCollapsable = false,
    onToggleNavbar,
    areSomeItemsDisabled = false,
}: NavbarProps) {
    return (
        <div
            className={cx('flex h-fit w-full', {
                'flex-col px-md py-xs sm:h-full sm:w-auto sm:px-none sm:py-xl': isCollapsable,
            })}
        >
            {isCollapsable && (
                <div className="flex w-full items-center justify-between sm:mb-[48px] sm:flex-col">
                    <div className="flex justify-center">
                        <IotaLogoMark
                            width={38}
                            height={38}
                            className="text-neutral-10 dark:text-neutral-92"
                        />
                    </div>
                    <div
                        className="state-layer relative rounded-full p-xs hover:cursor-pointer dark:text-neutral-92 sm:hidden"
                        onClick={onToggleNavbar}
                    >
                        <MenuIcon className={'h-6 w-6'} />
                    </div>
                </div>
            )}
            <div
                className={cx({
                    'flex w-full justify-between px-sm py-xxs': !isCollapsable,
                    'hidden sm:flex sm:flex-col sm:gap-2': isCollapsable,
                })}
            >
                {items.map((item) => (
                    <div
                        key={item.id}
                        className={cx('flex items-center', {
                            'px-xs py-xxs': !isCollapsable,
                            'py-xxs pl-xs pr-sm': isCollapsable,
                        })}
                    >
                        <NavbarItem
                            {...item}
                            isSelected={item.id === activeId}
                            onClick={(e) => {
                                if (!areSomeItemsDisabled) {
                                    if (item.onClick) {
                                        item.onClick(e);
                                    }
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
