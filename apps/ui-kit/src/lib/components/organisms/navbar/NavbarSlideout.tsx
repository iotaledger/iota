// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { NavbarItem, NavbarItemProps } from '@/components/molecules/navbar-item/NavbarItem';
import { NavbarProps } from './Navbar';

function NavbarSlideout({ items, activeId, onClick, className }: NavbarProps) {
    const handleBackClick = () => {
        console.info('Back button clicked');
    };

    return (
        <div className={cx(className)}>
            <div className={cx('')}>
                {items.map((item) => (
                    <div key={item.id} className=" px-xs py-xxs  ">
                        <NavbarItem {...item} />
                    </div>
                ))}
            </div>
        </div>
    );
}

export { NavbarSlideout };
