// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useContext } from 'react';
import { ArrowBack } from '@iota/ui-icons';
import cx from 'classnames';
import { Button, ButtonType, NavbarItem, NavbarItemType } from '@/components';
import { NavbarContext, ActionType } from './NavbarContext';
import { NavbarProps } from './Navbar';

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
                onClick={handleBackClick}
                className={cx('duration-800 transition-opacity ease-out', {
                    'opacity-1 fixed left-0 top-0 h-full w-full bg-shader-neutral-light-72':
                        state.isOpen,
                    '-translate-x-full opacity-0': !state.isOpen,
                })}
            />
            <div
                className={cx(
                    'z-999 rounded-tb-3xl fixed left-0 top-0 h-full w-11/12 rounded-tr-3xl bg-white px-lg py-lg transition-transform duration-300 ease-out dark:bg-neutral-6',
                    {
                        'translate-x-0': state.isOpen,
                        '-translate-x-full': !state.isOpen,
                    },
                )}
            >
                <div className="flex flex-col gap-2">
                    <div>
                        <Button
                            type={ButtonType.Ghost}
                            onClick={handleBackClick}
                            icon={<ArrowBack className="h-5 w-5" />}
                        />
                    </div>
                    {items.map((item) => (
                        <NavbarItem
                            key={item.id}
                            {...item}
                            type={NavbarItemType.Vertical}
                            isSelected={item.id === activeId}
                            onClick={() => {
                                onClickItem(item.id);
                                handleBackClick();
                            }}
                        />
                    ))}
                </div>
            </div>
        </>
    );
}
