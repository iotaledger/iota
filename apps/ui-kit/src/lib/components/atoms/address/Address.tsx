// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Copy, ArrowTopRight } from '@iota/ui-icons';
import { UnstyledButton } from '../button';

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
     * The onCopy event of the Address  (optional).
     */
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * The onOpen event of the Address  (optional).
     */
    onOpen?: (e: React.MouseEvent<HTMLButtonElement>) => void;
}

export function Address({
    text,
    isCopyable,
    isExternal,
    onCopy,
    onOpen,
}: AddressProps): React.JSX.Element {
    return (
        <div className="group flex flex-row items-center justify-center gap-1 text-neutral-40 dark:text-neutral-60">
            <span className={cx('font-inter text-body-sm')}>{text}</span>
            {isCopyable && (
                <UnstyledButton
                    onClick={onCopy}
                    className="opacity-0 focus:opacity-100 group-hover:opacity-100"
                >
                    <Copy />
                </UnstyledButton>
            )}
            {isExternal && (
                <UnstyledButton
                    onClick={onOpen}
                    className="opacity-0 focus:opacity-100 group-hover:opacity-100"
                >
                    <ArrowTopRight />
                </UnstyledButton>
            )}
        </div>
    );
}
