// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Tooltip, TooltipPosition } from '../../atoms';
import { Info } from '@iota/ui-icons';
import { DisplayStatsType, DisplayStatsSize } from './display-stats.enums';
import cx from 'classnames';
import {
    BACKGROUND_CLASSES,
    SIZE_CLASSES,
    TEXT_CLASSES,
    VALUE_TEXT_CLASSES,
    SUPPORTING_LABEL_TEXT_CLASSES,
    LABEL_TEXT_CLASSES,
} from './display-stats.classes';

interface DisplayStatsProps {
    /**
     * The label of the stats.
     */
    label: string;
    /**
     * The tooltip position.
     */
    tooltipPosition?: TooltipPosition;
    /**
     * The tooltip text.
     */
    tooltipText?: string;
    /**
     * The value of the stats.
     */
    value: string;
    /**
     * The supporting label of the stats (optional).
     */
    supportingLabel?: string;
    /**
     * The background color of the stats.
     */
    type?: DisplayStatsType;
    /**
     * The size of the stats.
     */
    size?: DisplayStatsSize;
    /**
     * Add icon to the right of the label.
     */
    icon?: React.ReactNode;
}

export function DisplayStats({
    label,
    tooltipPosition,
    tooltipText,
    value,
    supportingLabel,
    type = DisplayStatsType.Default,
    size = DisplayStatsSize.Default,
    icon,
}: DisplayStatsProps): React.JSX.Element {
    const backgroundClass = BACKGROUND_CLASSES[type];
    const sizeClass = SIZE_CLASSES[size];
    const textClass = TEXT_CLASSES[type];
    const valueTextClass = VALUE_TEXT_CLASSES[size];
    const labelTextClass = LABEL_TEXT_CLASSES[size];
    const supportingLabelTextClass = SUPPORTING_LABEL_TEXT_CLASSES[size];
    return (
        <div
            className={cx(
                'flex flex-col justify-between rounded-2xl p-md--rs',
                backgroundClass,
                sizeClass,
                textClass,
            )}
        >
            <div
                className={cx('flex flex-row items-center', {
                    'w-full justify-between': icon,
                })}
            >
                <div className="flex flex-row items-center gap-xxs">
                    <span className={cx(labelTextClass)}>{label}</span>
                    {tooltipText && (
                        <Tooltip text={tooltipText} position={tooltipPosition}>
                            <Info className="opacity-40" />
                        </Tooltip>
                    )}
                </div>
                {icon && <span className="text-neutral-10 dark:text-neutral-92">{icon}</span>}
            </div>
            <div className="flex flex-row items-baseline gap-xxs">
                <span className={cx(valueTextClass)}>{value}</span>
                {supportingLabel && (
                    <span className={cx('opacity-40', supportingLabelTextClass)}>
                        {supportingLabel}
                    </span>
                )}
            </div>
        </div>
    );
}
