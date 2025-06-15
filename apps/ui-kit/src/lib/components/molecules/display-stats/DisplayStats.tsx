// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ReactNode } from 'react';
import type { TooltipPosition } from '@/components/atoms';
import { ButtonUnstyled, Tooltip } from '@/components/atoms';
import { Copy, Info } from '@iota/apps-ui-icons';
import { DisplayStatsType, DisplayStatsSize } from './displayStats.enums';
import cx from 'classnames';
import {
    BACKGROUND_CLASSES,
    SIZE_CLASSES,
    TEXT_CLASSES,
    VALUE_TEXT_CLASSES,
    SUPPORTING_LABEL_TEXT_CLASSES,
    LABEL_TEXT_CLASSES,
} from './displayStats.classes';

interface DisplayStatsProps {
    /**
     * The label of the stats.
     */
    label: ReactNode;
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
    value: ReactNode;
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
    copyText,
    onCopySuccess,
    onCopyError,
}: DisplayStatsProps): React.JSX.Element {
    const backgroundClass = BACKGROUND_CLASSES[type];
    const sizeClass = SIZE_CLASSES[size];
    const textClass = TEXT_CLASSES[type];
    const valueClass = VALUE_TEXT_CLASSES[size];
    const labelClass = LABEL_TEXT_CLASSES[size];
    const supportingLabelTextClass = SUPPORTING_LABEL_TEXT_CLASSES[size];

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
                'flex h-full w-full flex-col justify-between rounded-2xl p-md--rs',
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
                    <span className={cx(labelClass, 'whitespace-pre-line')}>{label}</span>
                    {tooltipText && (
                        <Tooltip text={tooltipText} position={tooltipPosition}>
                            <Info className="opacity-40" />
                        </Tooltip>
                    )}
                </div>
                {icon && <span className="text-neutral-10 dark:text-neutral-92">{icon}</span>}
            </div>
            <div className="flex w-full flex-row items-baseline gap-xxs">
                <span className={cx('break-all', valueClass)}>{value}</span>
                {supportingLabel && (
                    <span className={cx('opacity-40', supportingLabelTextClass)}>
                        {supportingLabel}
                    </span>
                )}
                {copyText && (
                    <div className="self-center">
                        <ButtonUnstyled onClick={handleCopyClick}>
                            <Copy className="text-neutral-60 dark:text-neutral-40" />
                        </ButtonUnstyled>
                    </div>
                )}
            </div>
        </div>
    );
}
