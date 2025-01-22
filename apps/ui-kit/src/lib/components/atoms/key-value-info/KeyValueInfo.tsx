// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ReactNode } from 'react';
import cx from 'classnames';
import { Copy, Info } from '@iota/apps-ui-icons';
import { ValueSize } from './keyValue.enums';
import { Tooltip, TooltipPosition } from '../tooltip';
import { ButtonUnstyled } from '../button';

interface KeyValueProps {
    /**
     * The key of the KeyValue.
     */
    keyText: string;
    /**
     * The value of the KeyValue.
     */
    value: ReactNode;
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
     * Full width KeyValue (optional).
     */
    fullwidth?: boolean;
    /**
     * Reverse the KeyValue (optional).
     */
    isReverse?: boolean;
    /**
     * Text shown on value hover.
     */
    valueHoverTitle?: string;
}

export function KeyValueInfo({
    keyText,
    value,
    tooltipPosition,
    tooltipText,
    supportingLabel,
    size = ValueSize.Small,
    isTruncated = false,
    copyText,
    onCopySuccess,
    onCopyError,
    fullwidth,
    isReverse = false,
    valueHoverTitle,
}: KeyValueProps): React.JSX.Element {
    const flexDirectionClass = isReverse ? 'flex-row-reverse' : 'flex-row';
    async function handleCopyClick(event: React.MouseEvent<HTMLButtonElement>) {
        if (!navigator.clipboard) {
            return;
        }

        if (copyText) {
            try {
                await navigator.clipboard.writeText(copyText);
                onCopySuccess?.(event, copyText);
            } catch (error) {
                console.error('Failed to copy:', error);
                onCopyError?.(error, copyText);
            }
        }
    }

    return (
        <div
            className={cx(
                'flex w-full items-baseline gap-xs py-xxs font-inter',
                flexDirectionClass,
                {
                    'justify-between': fullwidth,
                },
            )}
        >
            <div
                className={cx('flex shrink-0 flex-row items-center gap-x-0.5', {
                    'w-1/4': !fullwidth,
                })}
            >
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">{keyText}</span>
                {tooltipText && (
                    <Tooltip text={tooltipText} position={tooltipPosition}>
                        <Info className="text-neutral-60 dark:text-neutral-40" />
                    </Tooltip>
                )}
            </div>
            <div
                className={cx('flex flex-row items-baseline gap-1 break-all', {
                    'w-3/4': !fullwidth,
                    truncate: isTruncated,
                })}
            >
                <span
                    title={valueHoverTitle}
                    className={cx(
                        'text-neutral-10 dark:text-neutral-92',
                        size === ValueSize.Medium ? 'text-body-lg' : 'text-body-md',
                        { truncate: isTruncated },
                    )}
                >
                    {value}
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
                <div className="self-center">
                    {copyText && (
                        <ButtonUnstyled onClick={handleCopyClick}>
                            <Copy className="text-neutral-60 dark:text-neutral-40" />
                        </ButtonUnstyled>
                    )}
                </div>
            </div>
        </div>
    );
}
