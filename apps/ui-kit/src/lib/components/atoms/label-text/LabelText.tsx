// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { LabelTextSize } from './labelText.enums';
import { LABEL_TEXT_SIZE, SUPPORTING_TEXT_SIZE, TEXT_SIZE } from './labelText.classes';
import { Info } from '@iota/apps-ui-icons';
import type { TooltipPosition } from '../tooltip';
import { Tooltip } from '../tooltip';

interface LabelTextProps {
    /**
     * The size of the LabelText.
     */
    size?: LabelTextSize;
    /**
     * The position of the LabelText.
     */
    isCentered?: boolean;
    /**
     * The supporting label of the LabelText.
     */
    supportingLabel?: string;
    /**
     * The text of the LabelText.
     */
    label: string;
    /**
     * The text of the LabelText.
     */
    text: string;
    /**
     * The tooltip position.
     */
    tooltipPosition?: TooltipPosition;
    /**
     * The tooltip text.
     */
    tooltipText?: string;
}

export function LabelText({
    size = LabelTextSize.Medium,
    isCentered,
    supportingLabel,
    label,
    text,
    tooltipPosition,
    tooltipText,
}: LabelTextProps): React.JSX.Element {
    const textClasses = TEXT_SIZE[size];
    const supportingLabelClasses = SUPPORTING_TEXT_SIZE[size];
    const labelTextClasses = LABEL_TEXT_SIZE[size];
    const centeredClasses = isCentered ? 'items-center' : 'items-start';
    const gapClass = size === LabelTextSize.Small ? 'gap-0.5' : 'gap-1';
    return (
        <div className={cx('flex flex-col', centeredClasses, gapClass)}>
            <div className="flex flex-row flex-wrap items-center gap-0.5">
                <span
                    className={cx('font-inter text-neutral-10 dark:text-neutral-92', textClasses)}
                >
                    {text}
                </span>
                {supportingLabel && (
                    <span
                        className={cx(
                            'font-inter text-neutral-60 dark:text-neutral-40',
                            supportingLabelClasses,
                        )}
                    >
                        {supportingLabel}
                    </span>
                )}
            </div>
            <div className="flex flex-row items-center gap-x-0.5">
                <span
                    className={cx(
                        'flex items-center font-inter text-neutral-60 dark:text-neutral-40',
                        labelTextClasses,
                    )}
                >
                    {label}
                    {tooltipText && (
                        <div className="ml-xxxs flex items-center">
                            <Tooltip text={tooltipText} position={tooltipPosition}>
                                <Info className="text-neutral-60 dark:text-neutral-40" />
                            </Tooltip>
                        </div>
                    )}
                </span>
            </div>
        </div>
    );
}
