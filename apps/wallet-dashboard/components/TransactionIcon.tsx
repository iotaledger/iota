// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TransactionAction, TransactionState } from '@/lib/interfaces';
import { IotaLogoMark, Right } from '@iota/icons';

const icons = {
    [TransactionAction.Send]: (
        <Right fill="currentColor" className="text-gradient-blue-start text-body -rotate-45" />
    ),
    [TransactionAction.Receive]: (
        <Right fill="currentColor" className="text-gradient-blue-start text-body rotate-135" />
    ),
    [TransactionAction.Transaction]: (
        <Right fill="currentColor" className="text-gradient-blue-start text-body -rotate-45" />
    ),
    [TransactionAction.Staked]: null,
    //     <WalletActionStake24 className="text-gradient-blue-start text-heading2 bg-transparent" />
    // ),
    [TransactionAction.Unstaked]: null,
    // <Unstaked className="text-gradient-blue-start text-heading3" />,
    [TransactionAction.Rewards]: <IotaLogoMark />,
    [TransactionAction.PersonalMessage]: null,
    //     <Account24 fill="currentColor" className="text-gradient-blue-start text-body" />
    // ),
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
