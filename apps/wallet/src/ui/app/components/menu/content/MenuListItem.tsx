// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ChevronRight16 } from '@iota/icons';
import clsx from 'clsx';
import type { MouseEventHandler, ReactNode } from 'react';
import { Link } from 'react-router-dom';

export interface ItemProps {
    icon: ReactNode;
    title: ReactNode;
    subtitle?: ReactNode;
    iconAfter?: ReactNode;
    to?: string;
    href?: string;
    onClick?: MouseEventHandler<Element>;
}

function MenuListItem({
    icon,
    title,
    subtitle,
    iconAfter,
    to = '',
    href = '',
    onClick,
}: ItemProps) {
    const Component = to ? Link : 'div';

    const MenuItemContent = (
        <>
            <div className="flex flex-1 basis-3/5 flex-nowrap items-center gap-2 overflow-hidden">
                <div className="text-steel text-2xl flex flex-none">{icon}</div>
                <div className="text-gray-90 flex flex-1 text-body font-semibold">{title}</div>
            </div>
            {subtitle || iconAfter || to ? (
                <div
                    className={clsx(
                        { 'flex-1 basis-2/5': Boolean(subtitle) },
                        'flex flex-nowrap items-center justify-end gap-1 overflow-hidden',
                    )}
                >
                    {subtitle ? (
                        <div className="text-steel-dark group-hover:text-steel-darker text-bodySmall font-medium transition">
                            {subtitle}
                        </div>
                    ) : null}
                    <div className="text-steel group-hover:text-steel-darker text-base flex flex-none transition">
                        {iconAfter || (to && <ChevronRight16 />) || null}
                    </div>
                </div>
            ) : null}
        </>
    );

    if (href) {
        return (
            <a
                href={href}
                target="_blank"
                rel="noreferrer noopener"
                className="px-1 py-4.5 first:pb-3 first:pt-3 last:pb-3 group flex cursor-pointer flex-nowrap items-center gap-5 overflow-hidden no-underline"
            >
                {MenuItemContent}
            </a>
        );
    }
    return (
        <Component
            data-testid={title}
            className="px-1 py-5 first:pb-3 first:pt-3 last:pb-3 group flex cursor-pointer flex-nowrap items-center gap-5 overflow-hidden no-underline"
            to={to}
            onClick={onClick}
        >
            {MenuItemContent}
        </Component>
    );
}

export default MenuListItem;
