// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';
import { ArrowDown } from '@iota/ui-icons';
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

export function AccordionHeader(props: PropsWithChildren<AccordionHeaderProps>) {
    return (
        <div
            onClick={props.onToggle}
            className="state-layer relative flex cursor-pointer items-center justify-between gap-8 rounded-xl pr-md--rs"
        >
            {props.children}
            <ArrowDown
                className={cx(ICON_STYLE, {
                    'rotate-180': props.isExpanded,
                })}
            />
        </div>
    );
}

export function AccordionContent(props: PropsWithChildren<AccordionContentProps>) {
    return (
        <div
            className={cx('border-box px-lg pb-md--rs pt-xs--rs', {
                hidden: !props.isExpanded,
            })}
        >
            {props.children}
        </div>
    );
}

export function Accordion({ children }: { children: React.ReactNode }): React.JSX.Element {
    return <div className="rounded-xl">{children}</div>;
}
