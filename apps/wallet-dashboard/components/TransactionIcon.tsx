// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionAction, TransactionState } from '@/lib/interfaces';
import { Account24, ArrowRight16, Iota, Unstaked, WalletActionStake24 } from '@iota/icons';

const icons = {
    [TransactionAction.Send]: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body -rotate-45"
        />
    ),
    [TransactionAction.Receive]: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body rotate-135"
        />
    ),
    [TransactionAction.Transaction]: (
        <ArrowRight16
            fill="currentColor"
            className="text-gradient-blue-start text-body -rotate-45"
        />
    ),
    [TransactionAction.Staked]: (
        <WalletActionStake24 className="text-gradient-blue-start text-heading2 bg-transparent" />
    ),
    [TransactionAction.Unstaked]: <Unstaked className="text-gradient-blue-start text-heading3" />,
    [TransactionAction.Rewards]: <Iota className="text-gradient-blue-start text-body" />,
    [TransactionAction.PersonalMessage]: (
        <Account24 fill="currentColor" className="text-gradient-blue-start text-body" />
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
