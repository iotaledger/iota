// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Info } from '@iota/ui-icons';
import { ValueSize } from './keyValue.enums';
import { SUPPORTING_LABEL_TEXT_SIZE } from './keyValueInfo.classes';

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
     * Show info icon (optional).
     */
    showInfoIcon?: boolean;
    /**
     * The supporting label of the KeyValue (optional).
     */
    supportingLabel?: string;
    /**
     * The size of the value (optional).
     */
    size?: ValueSize;
}

export function KeyValueInfo({
    keyText,
    valueText,
    showInfoIcon,
    supportingLabel,
    valueLink,
    size,
}: KeyValueProps): React.JSX.Element {
    const supportingLabelClasses = size && SUPPORTING_LABEL_TEXT_SIZE[size];
    return (
        <div className="flex w-full flex-row items-center justify-between gap-2 py-xxs font-inter">
            <div className="flex flex-row items-center">
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">{keyText}</span>
                {showInfoIcon && <Info className="pl-xxxs text-neutral-60" />}
            </div>
            <div className="flex w-full flex-row items-center justify-end gap-1">
                {valueLink ? (
                    <a
                        href={valueLink}
                        target="_blank"
                        rel="noreferrer"
                        className="text-body-md text-primary-60 dark:text-primary-40"
                    >
                        {valueText}
                    </a>
                ) : (
                    <>
                        <span
                            className={cx('text-neutral-10 dark:text-neutral-92', {
                                'text-body-md': size !== ValueSize.Medium,
                                'text-body-lg': size === ValueSize.Medium,
                            })}
                        >
                            {valueText}
                        </span>
                        {supportingLabel && (
                            <span
                                className={cx(
                                    'pt-xxs text-neutral-60 dark:text-neutral-40',
                                    supportingLabelClasses,
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
