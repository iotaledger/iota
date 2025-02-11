// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ArrowBack } from '@iota/apps-ui-icons';
import cx from 'classnames';
import { Button, ButtonType, NavbarItem, NavbarItemType } from '@/components';
import { NavbarProps } from './Navbar';

export function NavbarSlideout({
    items,
    activeId,
    onClickItem,
    isOpen,
    onToggleNavbar,
}: NavbarProps) {
    return (
        <>
            <div
                onClick={onToggleNavbar}
                className={cx('duration-800 transition-opacity ease-out', {
                    'opacity-1 fixed left-0 top-0 h-full w-full bg-shader-neutral-light-72': isOpen,
                    '-translate-x-full opacity-0': !isOpen,
                })}
            />
            <div
                className={cx(
                    'z-999 rounded-tb-3xl fixed left-0 top-0 h-full w-11/12 rounded-tr-3xl bg-white px-lg py-lg transition-transform duration-300 ease-out dark:bg-neutral-6',
                    {
                        'translate-x-0': isOpen,
                        '-translate-x-full': !isOpen,
                    },
                )}
            >
                <div className="flex flex-col gap-2">
                    <div className="[&_svg]:h-5 [&_svg]:w-5">
                        <Button
                            type={ButtonType.Ghost}
                            onClick={onToggleNavbar}
                            icon={<ArrowBack />}
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
                                onToggleNavbar && onToggleNavbar();
                            }}
                        />
                    ))}
                </div>
            </div>
        </>
    );
}
