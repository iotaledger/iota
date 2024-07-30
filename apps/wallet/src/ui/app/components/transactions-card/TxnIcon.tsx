// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Account24, ArrowRight16, Info16, Iota, Unstaked, WalletActionStake24 } from '@iota/icons';
import cl from 'clsx';

import LoadingIndicator from '../loading/LoadingIndicator';

const icons = {
    Send: (
        <ArrowRight16
            fill="currentColor"
            className="-rotate-45 text-body text-gradient-blue-start"
        />
    ),
    Receive: (
        <ArrowRight16
            fill="currentColor"
            className="rotate-135 text-body text-gradient-blue-start"
        />
    ),
    Transaction: (
        <ArrowRight16
            fill="currentColor"
            className="-rotate-45 text-body text-gradient-blue-start"
        />
    ),
    Staked: (
        <WalletActionStake24 className="bg-transparent text-heading2 text-gradient-blue-start" />
    ),
    Unstaked: <Unstaked className="text-heading3 text-gradient-blue-start" />,
    Rewards: <Iota className="text-body text-gradient-blue-start" />,
    Failed: <Info16 className="text-issue-dark text-heading6" />,
    Loading: <LoadingIndicator />,
    PersonalMessage: (
        <Account24 fill="currentColor" className="text-body text-gradient-blue-start" />
    ),
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
