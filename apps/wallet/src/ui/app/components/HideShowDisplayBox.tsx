// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonType, TextArea } from '@iota/apps-ui-kit';
import { toast } from '@iota/core';
import { ampli } from '_src/shared/analytics/ampli';

export interface HideShowDisplayBoxProps {
    value: string | string[];
    hideCopy?: boolean;
    copiedMessage?: string;
    isContentVisible?: boolean;
    eventType?: string;
}

export function HideShowDisplayBox({
    value,
    hideCopy = false,
    copiedMessage,
    isContentVisible = false,
    eventType = 'secrets',
}: HideShowDisplayBoxProps) {
    async function handleCopy() {
        if (!value) {
            return;
        }
        const textToCopy = Array.isArray(value) ? value.join(' ') : value;
        try {
            await navigator.clipboard.writeText(textToCopy);
            ampli.copiedElement({
                type: eventType,
            });
            toast(copiedMessage || 'Copied');
        } catch {
            toast.error('Failed to copy');
        }
    }

    return (
        <div className="flex flex-col gap-md" data-testid="mnemonic-display-box" data-amp-mask>
            <TextArea
                defaultValue={value}
                isVisibilityToggleEnabled
                isContentVisible={isContentVisible}
                rows={5}
            />
            {!hideCopy && (
                <div className="flex justify-end">
                    <Button onClick={handleCopy} type={ButtonType.Secondary} text="Copy" />
                </div>
            )}
        </div>
    );
}
