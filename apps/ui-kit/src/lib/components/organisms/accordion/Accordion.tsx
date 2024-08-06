// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { PropsWithChildren } from 'react';
import cx from 'classnames';
import { Badge, BadgeType } from '../../atoms';
import { Title } from '@/lib';
import { ArrowDown, ArrowUp } from '@iota/ui-icons';

interface AccordionProps {
    /**
     * Flag for show/hide content
     */
    isExpanded: boolean;

    /**
     * Action on toggle show/hide content
     */
    onToggle: () => void;

    /**
     * Text for title.
     */
    title: string;

    /**
     * Text for subtitle.
     */
    subtitle?: string;

    /**
     * The type of the badge.
     */
    badgeType?: BadgeType;
    /**
     * The text of the badge.
     */
    badgeText?: string;
}

export function Accordion({
    isExpanded,
    onToggle,
    title,
    subtitle,
    badgeType,
    badgeText,
    children,
}: PropsWithChildren<AccordionProps>): React.JSX.Element {
    const badge = (() => {
        if (!badgeText || !badgeType) {
            return;
        }
        return <Badge type={badgeType} label={badgeText} />;
    })();

    const arrow = (() => {
        if (isExpanded) {
            return <ArrowUp className="h-5 w-5" />;
        }
        return <ArrowDown className="h-5 w-5" />;
    })();

    return (
        <div className="rounded-xl">
            <div onClick={onToggle} className="state-layer relative cursor-pointer rounded-xl">
                <Title
                    title={title}
                    subtitle={subtitle}
                    supportingElement={badge}
                    trailingElement={arrow}
                />
            </div>
            <div
                className={cx('border-box px-lg pb-md--rs pt-xs--rs', {
                    hidden: !isExpanded,
                })}
            >
                {children}
            </div>
        </div>
    );
}
