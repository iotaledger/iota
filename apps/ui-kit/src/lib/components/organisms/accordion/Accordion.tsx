// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PropsWithChildren } from 'react';
import cx from 'classnames';
import { ArrowDown } from '@iota/apps-ui-icons';
import { Button, ButtonType } from '@/lib';
import { ICON_STYLE } from './accordion.classes';

export interface AccordionHeaderProps {
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
    /**
     * Flag for hiding arrow.
     */
    hideArrow?: boolean;
    /**
     * Flag for hiding border.
     */
    hideBorder?: boolean;
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
    hideArrow,
    hideBorder,
}: PropsWithChildren<AccordionHeaderProps>) {
    return (
        <div
            onClick={onToggle}
            className={cx(
                'state-layer relative flex cursor-pointer items-center justify-between gap-md py-sm--rs',
                {
                    'pr-md--rs': !hideArrow,
                    'rounded-xl': hideBorder,
                },
            )}
        >
            {children}
            {!hideArrow && (
                <div className="[&_svg]:h-5 [&_svg]:w-5">
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
            )}
        </div>
    );
}

export function AccordionContent({
    isExpanded,
    children,
}: PropsWithChildren<AccordionContentProps>) {
    return (
        <div
            className={cx({
                hidden: !isExpanded,
            })}
        >
            {children}
        </div>
    );
}

export function Accordion({
    children,
    hideBorder,
}: {
    children: React.ReactNode;
    hideBorder?: boolean;
}): React.JSX.Element {
    return (
        <div
            className={cx('overflow-hidden rounded-xl bg-neutral-100 dark:bg-neutral-6', {
                ' border border-shader-neutral-light-8 dark:border-shader-neutral-dark-8':
                    !hideBorder,
            })}
        >
            {children}
        </div>
    );
}
