// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';
import { Copy, ArrowTopRight } from '@iota/ui-icons';

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
    onCopy?: (e: React.MouseEvent<SVGElement>) => void;
    /**
     * The onOpen event of the Address  (optional).
     */
    onOpen?: (e: React.MouseEvent<SVGElement>) => void;
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
                <Copy className="hidden cursor-pointer group-hover:flex" onClick={onCopy} />
            )}
            {isExternal && (
                <ArrowTopRight
                    className="hidden cursor-pointer group-hover:flex"
                    onClick={onOpen}
                />
            )}
        </div>
    );
}
