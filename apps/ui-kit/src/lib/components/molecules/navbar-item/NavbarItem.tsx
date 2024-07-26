// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { NavbarItemHorizontal } from './NavbarItemHorizontal';
import { NavbarItemVertical } from './NavbarItemVertical';
import { NavbarItemType } from './navbarItem.enums';

export interface NavbarItemProps {
    /**
     * The type of the navbar item.
     */
    type?: NavbarItemType;

    /**
     * The icon of the navbar item.
     */
    icon: React.ReactNode;
    /**
     * The text of the navbar item.
     */
    text?: string;
    /**
     * Indicates if the navbar item is selected.
     */
    isSelected?: boolean;
    /**
     * Indicates if the navbar item has a badge.
     */
    hasBadge?: boolean;
    /**
     * The onClick event of the navbar item.
     */
    onClick?: (e: React.MouseEvent<HTMLDivElement>) => void;
}

export function NavbarItem(props: NavbarItemProps): React.JSX.Element {
    const { type } = props;

    if (type === NavbarItemType.Horizontal) {
        return <NavbarItemHorizontal {...props} />;
    }

    return <NavbarItemVertical {...props} />;
}
