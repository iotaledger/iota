// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Info } from '@iota/ui-icons';
import { ValueSize } from './keyValue.enums';

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
    size = ValueSize.Small,
}: KeyValueProps): React.JSX.Element {
    return (
        <div className="flex w-full flex-row items-center justify-between gap-2 py-xxs font-inter">
            <div className="flex flex-row items-center gap-x-0.5">
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">{keyText}</span>
                {showInfoIcon && <Info className="pl-xxxs text-neutral-60" />}
            </div>
            <div className="flex w-full flex-row items-baseline justify-end gap-1">
                {valueLink ? (
                    <a
                        href={valueLink}
                        target="_blank"
                        rel="noreferrer"
                        className="text-body-md text-primary-30 dark:text-primary-80"
                    >
                        {valueText}
                    </a>
                ) : (
                    <>
                        <span
                            className={cx(
                                'text-neutral-10 dark:text-neutral-92',
                                size === ValueSize.Medium ? 'text-body-lg' : 'text-body-md',
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
