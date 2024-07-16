// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { LabelTextSize } from './labelText.enums';
import { LABEL_TEXT_SIZE, SUPPORTING_TEXT_SIZE, VALUE_TEXT_SIZE } from './labelText.classes';

interface LabelTextProps {
    /**
     * The size of the LabelText.
     */
    size: LabelTextSize;
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
     * Show the supporting label.
     */
    showSupportingLabel: boolean;
    /**
     * The value of the LabelText.
     */
    value: string;
}

export function LabelText({
    size,
    isCentered,
    supportingLabel,
    label: text,
    showSupportingLabel,
    value,
}: LabelTextProps): React.JSX.Element {
    const valueTextClasses = VALUE_TEXT_SIZE[size];
    const supportingLabelClasses = SUPPORTING_TEXT_SIZE[size];
    const labelTextClasses = LABEL_TEXT_SIZE[size];
    const centeredClasses = isCentered ? 'items-center' : 'items-start';
    const gapClass = size === LabelTextSize.Small ? 'gap-0.5' : 'gap-1';
    return (
        <div className={cx('flex flex-col', centeredClasses, gapClass)}>
            <div className="flex flex-row items-center gap-0.5">
                <span
                    className={cx(
                        'font-inter text-neutral-10 dark:text-neutral-92',
                        valueTextClasses,
                    )}
                >
                    {value}
                </span>
                {showSupportingLabel && supportingLabel && (
                    <span
                        className={cx(
                            'pb-xxxs font-inter text-neutral-60 dark:text-neutral-40',
                            supportingLabelClasses,
                        )}
                    >
                        {supportingLabel}
                    </span>
                )}
            </div>
            <span
                className={cx('font-inter text-neutral-60 dark:text-neutral-40', labelTextClasses)}
            >
                {text}
            </span>
        </div>
    );
}
