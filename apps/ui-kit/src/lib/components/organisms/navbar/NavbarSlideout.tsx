// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { ArrowBack } from '@iota/ui-icons';
import cx from 'classnames';
import { NavbarItem } from '@/components/molecules/navbar-item/NavbarItem';
import { NavbarProps } from './Navbar';
import { NavbarItemType } from '@/components/molecules/navbar-item/navbarItem.enums';

function NavbarSlideout({
    items,
    activeId,
    onClick,
    isOpen,
    className,
}: NavbarProps & { isOpen: boolean }) {
    const handleBackClick = () => {
        console.info('Back button clicked');
    };

    return (
        <>
            <div className="z-1 absolute left-0 top-0 h-full w-full bg-shader-neutral-light-72"></div>
            <div
                className={cx(
                    className,
                    'z-2 rounded-tb-3xl absolute left-0 top-0 h-full w-9/12 rounded-tr-3xl bg-white py-lg transition-transform duration-300 ease-out',
                    {
                        'translate-x-0': isOpen,
                        '-translate-x-full': !isOpen,
                    },
                )}
            >
                <div className={cx('flex-col gap-1')}>
                    <div className="px-lg">
                        <div className={'cursor-pointer p-xs'} onClick={handleBackClick}>
                            <ArrowBack width={20} height={20} />
                        </div>
                    </div>
                    {items.map((item) => (
                        <div
                            key={item.id}
                            className={cx('state-layer relative flex cursor-pointer px-lg', {
                                'bg-primary-60 bg-opacity-8': item.id === activeId,
                            })}
                            onClick={() => onClick(item.id)}
                        >
                            <NavbarItem {...item} type={NavbarItemType.Horizontal} />
                        </div>
                    ))}
                </div>
            </div>
        </>
    );
}

export { NavbarSlideout };
