// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Copy, ArrowTopRight } from '@iota/ui-icons';
import { ButtonUnstyled } from '../../atoms/button/ButtonUnstyled';

interface AddressProps {
    /**
     * The text of the address.
     */
    text: string;
    /**
     * Has copy icon (optional).
     */
    isCopyable?: boolean;
    /**
     * Has open icon  (optional).
     */
    isExternal?: boolean;
    /**
     * Text that need to be copied (optional).
     */
    copyText?: string;
    /**
     * The onCopy event of the Address  (optional).
     */
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>, value: string) => void;
    /**
     * The onOpen event of the Address  (optional).
     */
    onOpen?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function Address({
    text,
    isCopyable,
    isExternal,
    copyText = text,
    onCopy,
    onOpen,
}: AddressProps): React.JSX.Element {
    async function handleCopyClick(event: React.MouseEvent<HTMLButtonElement>) {
        if (!navigator.clipboard) {
            return;
        }

        try {
            await navigator.clipboard.writeText(text);
            onCopy?.(event, copyText);
            return true;
        } catch (error) {
            console.error('Failed to copy:', error);
        }
    }

    return (
        <div className="group flex flex-row items-center gap-1 text-neutral-40 dark:text-neutral-60">
            <span className={cx('font-inter text-body-sm')}>{text}</span>
            {isCopyable && (
                <ButtonUnstyled
                    onClick={handleCopyClick}
                    className="opacity-0 focus:opacity-100 group-hover:opacity-100"
                >
                    <Copy />
                </ButtonUnstyled>
            )}
            {isExternal && (
                <ButtonUnstyled
                    onClick={onOpen}
                    className="opacity-0 focus:opacity-100 group-hover:opacity-100"
                >
                    <ArrowTopRight />
                </ButtonUnstyled>
            )}
        </div>
    );
}
