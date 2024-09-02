// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { Button, ButtonType, TextArea } from '@iota/apps-ui-kit';
import toast from 'react-hot-toast';

export interface HideShowDisplayBoxProps {
    value: string | string[];
    hideCopy?: boolean;
    copiedMessage?: string;
    isContentVisible?: boolean;
}

export function HideShowDisplayBox({
    value,
    hideCopy = false,
    copiedMessage,
    isContentVisible = false,
}: HideShowDisplayBoxProps) {
    const [valueCopied, setValueCopied] = useState(false);

    async function handleCopy() {
        if (!value) {
            return;
        }
        const textToCopy = Array.isArray(value) ? value.join(' ') : value;
        try {
            await navigator.clipboard.writeText(textToCopy);
            setValueCopied(true);
            setTimeout(() => {
                setValueCopied(false);
            }, 1000);
            toast.success(copiedMessage || 'Copied');
        } catch {
            toast.error('Failed to copy');
        }
    }

    return (
        <div className="flex flex-col gap-md">
            <TextArea
                value={value}
                isVisibilityToggleEnabled
                isContentVisible={isContentVisible}
                rows={5}
            />
            {!hideCopy && (
                <div className="flex justify-end">
                    <Button
                        onClick={handleCopy}
                        type={ButtonType.Secondary}
                        text={valueCopied ? 'Copied' : 'Copy'}
                    />
                </div>
            )}
        </div>
    );
}
