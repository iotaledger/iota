// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Copy, Info } from '@iota/ui-icons';
import { ValueSize } from './keyValue.enums';
import { Tooltip, TooltipPosition } from '../tooltip';
import { ButtonUnstyled } from '../button';

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
     * Text that need to be copied (optional).
     */
    copyText?: string;
    /**
     * The onCopySuccess event of the KeyValue  (optional).
     */
    onCopySuccess?: (e: React.MouseEvent<HTMLButtonElement>, text: string) => void;
    /**
     * The onCopyError event of the KeyValue  (optional).
     */
    onCopyError?: (e: unknown, text: string) => void;
    /**
     * Has copy icon (optional).
     */
    isCopyable?: boolean;
    /**
     * Full width KeyValue (optional).
     */
    fullwidth?: boolean;
}

export function KeyValueInfo({
    keyText,
    valueText,
    tooltipPosition,
    tooltipText,
    supportingLabel,
    valueLink,
    size = ValueSize.Small,
    copyText = valueText,
    onCopySuccess,
    onCopyError,
    isCopyable,
    fullwidth,
}: KeyValueProps): React.JSX.Element {
    async function handleCopyClick(event: React.MouseEvent<HTMLButtonElement>) {
        if (!navigator.clipboard) {
            return;
        }

        try {
            await navigator.clipboard.writeText(copyText);
            onCopySuccess?.(event, copyText);
        } catch (error) {
            console.error('Failed to copy:', error);
            onCopyError?.(error, copyText);
        }
    }

    return (
        <div
            className={cx('flex w-full flex-row items-baseline gap-xs py-xxs font-inter', {
                'justify-between': fullwidth,
            })}
        >
            <div className="flex shrink-0 flex-row items-center gap-x-0.5">
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">{keyText}</span>
                {tooltipText && (
                    <Tooltip text={tooltipText} position={tooltipPosition}>
                        <Info className="text-neutral-60 dark:text-neutral-40" />
                    </Tooltip>
                )}
            </div>
            <div className="flex flex-row items-baseline gap-1 break-words">
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
                <div className="self-center">
                    {isCopyable && (
                        <ButtonUnstyled onClick={handleCopyClick}>
                            <Copy className="text-neutral-60 dark:text-neutral-40" />
                        </ButtonUnstyled>
                    )}
                </div>
            </div>
        </div>
    );
}

export default KeyValueInfo;
