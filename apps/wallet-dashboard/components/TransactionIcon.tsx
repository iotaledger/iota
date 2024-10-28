// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionAction, TransactionState } from '@/lib/interfaces';
import { ArrowRight, IotaLogoMark, Person, Stake, Unstake } from '@iota/ui-icons';

const icons = {
    [TransactionAction.Send]: (
        <ArrowRight fill="currentColor" className="text-gradient-blue-start text-body -rotate-45" />
    ),
    [TransactionAction.Receive]: (
        <ArrowRight fill="currentColor" className="text-gradient-blue-start text-body rotate-135" />
    ),
    [TransactionAction.Transaction]: (
        <ArrowRight fill="currentColor" className="text-gradient-blue-start text-body -rotate-45" />
    ),
    [TransactionAction.Staked]: (
        <Stake className="text-gradient-blue-start text-heading2 bg-transparent" />
    ),
    [TransactionAction.Unstaked]: <Unstake className="text-gradient-blue-start text-heading3" />,
    [TransactionAction.Rewards]: <IotaLogoMark className="text-gradient-blue-start text-body" />,
    [TransactionAction.PersonalMessage]: (
        <Person fill="currentColor" className="text-gradient-blue-start text-body" />
    ),
};

interface TransactionIconProps {
    state: TransactionState;
    action: keyof typeof icons;
}

function TransactionIcon({ state, action }: TransactionIconProps) {
    const isError = state === TransactionState.Failed;
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

export default TransactionIcon;
