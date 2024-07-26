// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from 'react';
import cx from 'classnames';
import { Copy, ArrowTopRight, Checkmark } from '@iota/ui-icons';

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
    const [isCopied, setIsCopied] = useState(false);

    const copy = (() => {
        if (!isCopyable) {
            return;
        }

        if (isCopied) {
            return <Checkmark />;
        }

        return (
            <Copy
                className="invisible cursor-pointer group-hover:visible"
                onClick={(e) => {
                    onCopy && onCopy(e);
                    setIsCopied(true);

                    setTimeout(() => {
                        setIsCopied(false);
                    }, 2000);
                }}
            />
        );
    })();

    return (
        <div className="group flex flex-row items-center justify-center gap-1 text-neutral-40 dark:text-neutral-60">
            <span className={cx('font-inter text-body-sm')}>{text}</span>
            {copy}
            {isExternal && (
                <ArrowTopRight
                    className="invisible cursor-pointer group-hover:visible"
                    onClick={onOpen}
                />
            )}
        </div>
    );
}
