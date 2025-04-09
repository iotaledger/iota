// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { Copy, ArrowTopRight } from '@iota/apps-ui-icons';
import { ButtonUnstyled } from '../../atoms/button';

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
     * The link for external resource (optional).
     */
    externalLink?: string;

    /**
     * Text that need to be copied (optional).
     */
    copyText?: string;
    /**
     * The onCopySuccess event of the Address  (optional).
     */
    onCopySuccess?: (e: React.MouseEvent<HTMLButtonElement>, text: string) => void;
    /**
     * The onCopyError event of the Address  (optional).
     */
    onCopyError?: (e: unknown, text: string) => void;
    /**
     * The onOpen event of the Address  (optional).
     */
    onOpen?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function Address({
    text,
    isCopyable,
    isExternal,
    externalLink,
    copyText = text,
    onCopySuccess,
    onCopyError,
    onOpen,
}: AddressProps): React.JSX.Element {
    async function handleCopyClick(event: React.MouseEvent<HTMLButtonElement>) {
        if (!navigator.clipboard) {
            return;
        }

        event?.stopPropagation();
        try {
            await navigator.clipboard.writeText(copyText);
            onCopySuccess?.(event, copyText);
        } catch (error) {
            console.error('Failed to copy:', error);
            onCopyError?.(error, copyText);
        }
    }

    function handleOpenClick(event: React.MouseEvent<HTMLButtonElement>) {
        event?.stopPropagation();
        if (externalLink) {
            const newWindow = window.open(externalLink, '_blank', 'noopener noreferrer');
            if (newWindow) newWindow.opener = null;
        } else {
            onOpen?.(event);
        }
    }

    return (
        <div className="group flex flex-row items-center gap-1 text-neutral-40 dark:text-neutral-60">
            <span className={cx('font-inter text-body-sm')}>{text}</span>
            {isCopyable && (
                <ButtonUnstyled
                    onClick={handleCopyClick}
                    className="opacity-0 group-hover:opacity-100"
                >
                    <Copy />
                </ButtonUnstyled>
            )}
            {isExternal && (
                <ButtonUnstyled
                    onClick={handleOpenClick}
                    className="opacity-0 group-hover:opacity-100"
                >
                    <ArrowTopRight />
                </ButtonUnstyled>
            )}
        </div>
    );
}
