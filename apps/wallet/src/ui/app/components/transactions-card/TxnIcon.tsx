// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Info } from '@iota/icons';
import cl from 'clsx';

const icons = {
    Send: null,
    Receive: null,
    Transaction: null,
    Staked: null,
    Unstaked: null,
    Rewards: null,
    Failed: <Info className="text-heading6 text-issue-dark" />,
    Loading: null,
    PersonalMessage: null,
};

interface TxnItemIconProps {
    txnFailed?: boolean;
    variant: keyof typeof icons;
}

export function TxnIcon({ txnFailed, variant }: TxnItemIconProps) {
    return (
        <div
            className={cl([
                txnFailed ? 'bg-issue-light' : 'bg-gray-40',
                'flex h-7.5 w-7.5 items-center justify-center rounded-2lg',
            ])}
        >
            {icons[txnFailed ? 'Failed' : variant]}
        </div>
    );
}
