// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { AddressType } from './address.enums';
import { TEXT_COLORS } from './address.classes';
import cx from 'classnames';

interface AddressProps {
    /**
     * The text of the address.
     */
    text: string;
    /**
     * The type of address
     */
    type?: AddressType;
    /**
     * The Copy icon
     */
    showCopy?: boolean;
    /**
     * Show Open icon
     */
    showOpen?: boolean;
    /**
     * The onClick event of the Address.
     */
    onClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * The onCopy event of the Address.
     */
    onCopy?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * The onOpen event of the Address.
     */
    onOpen?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    /**
     * Wheter to use the darkmode theme in this component.
     */
    darkmode?: boolean;
}

export function Address({
    text,
    showCopy,
    showOpen,
    onClick,
    onCopy,
    onOpen,
    darkmode,
    type = AddressType.Primary,
}: AddressProps): React.JSX.Element {
    // const paddingClasses = icon && !text ? PADDINGS_ONLY_ICON[size] : PADDINGS[size];
    // const backgroundColors = disabled ? DISABLED_BACKGROUND_COLORS[type] : BACKGROUND_COLORS[type];
    const textColors = TEXT_COLORS[type];
    return (
        <div className="flex flex-row items-center justify-center gap-2">
            <span onClick={onClick} className={cx('font-inter', textColors)}>
                {text}
            </span>
            {showCopy && (
                <span onClick={onCopy} className={cx(textColors)}>
                    copy
                </span>
            )}
            {showOpen && (
                <span onClick={onOpen} className={cx(textColors)}>
                    open
                </span>
            )}
        </div>
    );
}
