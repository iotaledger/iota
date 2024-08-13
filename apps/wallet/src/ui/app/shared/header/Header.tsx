// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Network } from '@iota/iota-sdk/client';
import { type ReactNode } from 'react';
import { Link } from 'react-router-dom';

import { Logo } from '_components';

interface HeaderProps {
    network: Network;
    middleContent?: ReactNode;
    rightContent?: ReactNode;
}

/**
 * General page header that can render arbitrary content where the content
 * located in the middle of the header is centered and has a capped width
 */
export function Header({ network, middleContent, rightContent }: HeaderProps) {
    return (
        <header className="grid grid-cols-header items-center gap-3 px-3 py-2">
            <div>
                <Link to="/" className="text-gray-90 no-underline">
                    <Logo network={network} />
                </Link>
            </div>
            {middleContent && <div className="col-start-2 overflow-hidden">{middleContent}</div>}
            {rightContent && (
                <div className="col-start-3 mr-1 justify-self-end">{rightContent}</div>
            )}
        </header>
    );
}
