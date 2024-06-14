// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ActivityAction, ActivityState } from '@/lib/interfaces';
import { Account24, ArrowRight16, Iota, Unstaked, WalletActionStake24 } from '@iota/icons';

const icons = {
    [ActivityAction.Send]: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body -rotate-45"
        />
    ),
    [ActivityAction.Receive]: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body rotate-135"
        />
    ),
    [ActivityAction.Transaction]: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body -rotate-45"
        />
    ),
    [ActivityAction.Staked]: (
        <WalletActionStake24 className="text-gradient-blue-start text-heading2 bg-transparent" />
    ),
    [ActivityAction.Unstaked]: <Unstaked className="text-gradient-blue-start text-heading3" />,
    [ActivityAction.Rewards]: <Iota className="text-gradient-blue-start text-body" />,
    [ActivityAction.PersonalMessage]: (
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
