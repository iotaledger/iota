// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useContext } from 'react';
import { ArrowBack } from '@iota/ui-icons';
import cx from 'classnames';
import { NavbarItem } from '@/components/molecules/navbar-item/NavbarItem';
import { NavbarProps } from './Navbar';
import { NavbarItemType } from '@/components/molecules/navbar-item/navbarItem.enums';
import { NavbarContext, ActionType } from '@/components/organisms/navbar/NavbarContext';

export function NavbarSlideout({ items, activeId, onClickItem }: NavbarProps) {
    const { state, dispatch } = useContext(NavbarContext);

    const handleBackClick = () => {
        dispatch({
            type: ActionType.ToggleNavbarOpen,
        });
    };

    return (
        <>
            <div
                className={cx({
                    'fixed left-0 top-0 h-full w-full bg-shader-neutral-light-72': state.isOpen,
                })}
            />
            <div
                className={cx(
                    'z-999 rounded-tb-3xl fixed left-0 top-0 h-full w-9/12 rounded-tr-3xl bg-white px-lg py-lg transition-transform duration-300 ease-out dark:bg-neutral-6',
                    {
                        'translate-x-0': state.isOpen,
                        '-translate-x-full': !state.isOpen,
                    },
                )}
            >
                <div className="flex flex-col gap-1">
                    <div
                        className="cursor-pointer p-xs dark:text-neutral-60"
                        onClick={handleBackClick}
                    >
                        <ArrowBack width={20} height={20} />
                    </div>
                    {items.map((item) => (
                        <NavbarItem
                            key={item.id}
                            {...item}
                            type={NavbarItemType.Vertical}
                            isSelected={item.id === activeId}
                            onClick={() => onClickItem(item.id)}
                        />
                    ))}
                </div>
            </div>
        </>
    );
}
