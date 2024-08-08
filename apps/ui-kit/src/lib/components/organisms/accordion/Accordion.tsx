// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';
import { ArrowDown } from '@iota/ui-icons';
import { Button, ButtonType } from '@/lib';
import { ICON_STYLE } from './accordion.classes';

interface AccordionHeaderProps {
    /**
     * Flag for show/hide content
     */
    isExpanded: boolean;

    /**
     * Action on toggle show/hide content
     */
    onToggle: () => void;

    /**
     * The type of the badge.
     */
    badge?: React.ReactNode;
}

interface AccordionContentProps {
    /**
     * Flag for show/hide content
     */
    isExpanded: boolean;
}

export function AccordionHeader({
    onToggle,
    children,
    isExpanded,
}: PropsWithChildren<AccordionHeaderProps>) {
    return (
        <div
            onClick={onToggle}
            className="state-layer relative flex cursor-pointer items-center justify-between gap-md py-sm--rs pr-md--rs"
        >
            {children}
            <Button
                type={ButtonType.Ghost}
                icon={
                    <ArrowDown
                        className={cx(ICON_STYLE, {
                            'rotate-180': isExpanded,
                        })}
                    />
                }
            />
        </div>
    );
}

export function AccordionContent({
    isExpanded,
    children,
}: PropsWithChildren<AccordionContentProps>) {
    return (
        <div
            className={cx('px-lg pb-md pt-xs', {
                hidden: !isExpanded,
            })}
        >
            {children}
        </div>
    );
}

export function Accordion({ children }: { children: React.ReactNode }): React.JSX.Element {
    return (
        <div className="rounded-xl border border-shader-inverted-dark-8 bg-neutral-100 dark:bg-neutral-6">
            {children}
        </div>
    );
}
