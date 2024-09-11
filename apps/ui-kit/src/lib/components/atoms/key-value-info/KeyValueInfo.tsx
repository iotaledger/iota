// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Info } from '@iota/ui-icons';
import { ValueSize } from './keyValue.enums';
import { Tooltip, TooltipPosition } from '../tooltip';

interface KeyValueProps {
    /**
     * The key of the KeyValue.
     */
    keyText: string;
    /**
     * The value text of the KeyValue.
     */
    valueText: string;
    /**
     * The value link of the KeyValue.
     */
    valueLink?: string;
    /**
     * The tooltip position.
     */
    tooltipPosition?: TooltipPosition;
    /**
     * The tooltip text.
     */
    tooltipText?: string;
    /**
     * The supporting label of the KeyValue (optional).
     */
    supportingLabel?: string;
    /**
     * The size of the value (optional).
     */
    size?: ValueSize;
    /**
     * The flag to truncate the value text.
     */
    isTruncated?: boolean;
}

export function KeyValueInfo({
    keyText,
    valueText,
    tooltipPosition,
    tooltipText,
    supportingLabel,
    valueLink,
    size = ValueSize.Small,
    isTruncated = false,
}: KeyValueProps): React.JSX.Element {
    return (
        <div className="flex w-full flex-row items-center justify-between gap-2 py-xxs font-inter">
            <div className="flex w-full flex-row items-center gap-x-0.5">
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">{keyText}</span>
                {tooltipText && (
                    <Tooltip text={tooltipText} position={tooltipPosition}>
                        <Info className="text-neutral-60 dark:text-neutral-40" />
                    </Tooltip>
                )}
            </div>
            <div
                className={cx('flex w-full flex-row items-baseline justify-end gap-1', {
                    truncate: isTruncated,
                })}
            >
                {valueLink ? (
                    <a
                        href={valueLink}
                        target="_blank"
                        rel="noreferrer"
                        className={cx('text-body-md text-primary-30 dark:text-primary-80', {
                            truncate: isTruncated,
                        })}
                    >
                        {valueText}
                    </a>
                ) : (
                    <>
                        <span
                            className={cx(
                                'text-neutral-10 dark:text-neutral-92',
                                size === ValueSize.Medium ? 'text-body-lg' : 'text-body-md',
                                { truncate: isTruncated },
                            )}
                        >
                            {valueText}
                        </span>
                        {supportingLabel && (
                            <span
                                className={cx(
                                    'text-neutral-60 dark:text-neutral-40',
                                    size === ValueSize.Medium ? 'text-body-md' : 'text-body-sm',
                                )}
                            >
                                {supportingLabel}
                            </span>
                        )}
                    </>
                )}
            </div>
        </div>
    );
}

export default KeyValueInfo;
