// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ActivityState } from '@/lib/interfaces';
import { Account24, ArrowRight16, Iota, Swap16, Unstaked, WalletActionStake24 } from '@iota/icons';

const icons = {
    Send: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body -rotate-45"
        />
    ),
    Receive: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body rotate-135"
        />
    ),
    Transaction: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body -rotate-45"
        />
    ),
    Staked: (
        <WalletActionStake24 className="text-gradient-blue-start text-heading2 bg-transparent" />
    ),
    Unstaked: <Unstaked className="text-gradient-blue-start text-heading3" />,
    Rewards: <Iota className="text-gradient-blue-start text-body" />,
    Swapped: <Swap16 className="text-gradient-blue-start text-heading6" />,
    PersonalMessage: (
        <Account24 fill="currentColor" className="text-gradient-blue-start text-body" />
    ),
};

interface ActivityIconProps {
    state: ActivityState;
    action: keyof typeof icons;
}

function ActivityIcon({ state, action }: ActivityIconProps) {
    const isError = state === ActivityState.Failed;
    return (
        <div
            className={`${
                isError ? 'bg-issue-light' : 'bg-gray-40'
            } w-7.5 h-7.5 rounded-2lg flex items-center justify-center`}
        >
            {icons[action]}
        </div>
    );
}

export default ActivityIcon;
