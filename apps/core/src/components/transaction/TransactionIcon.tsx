// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ArrowBottomLeft,
    ArrowTopRight,
    Info,
    Migration,
    Person,
    Stake,
    Unstake,
    Vesting,
} from '@iota/apps-ui-icons';
import { TransactionAction } from '../../interfaces';

const ICON_COLORS = {
    primary: 'text-primary-30',
    error: 'text-error-30',
};

const icons = {
    [TransactionAction.Send]: <ArrowTopRight className={ICON_COLORS.primary} />,
    [TransactionAction.Receive]: <ArrowBottomLeft className={ICON_COLORS.primary} />,
    [TransactionAction.Transaction]: <ArrowTopRight className={ICON_COLORS.primary} />,
    [TransactionAction.Staked]: <Stake className={ICON_COLORS.primary} />,
    [TransactionAction.Unstaked]: <Unstake className={ICON_COLORS.primary} />,
    [TransactionAction.Failed]: <Info className={ICON_COLORS.error} />,
    [TransactionAction.PersonalMessage]: <Person className={ICON_COLORS.primary} />,
    [TransactionAction.TimelockedStaked]: <Stake className={ICON_COLORS.primary} />,
    [TransactionAction.TimelockedUnstaked]: <Unstake className={ICON_COLORS.primary} />,
    [TransactionAction.Migration]: <Migration className={ICON_COLORS.primary} />,
    [TransactionAction.TimelockedCollect]: <Vesting className={ICON_COLORS.primary} />,
};

interface TransactionIconProps {
    txnFailed?: boolean;
    variant: TransactionAction;
}

export function TransactionIcon({ txnFailed, variant }: TransactionIconProps) {
    return (
        <div className="[&_svg]:h-5 [&_svg]:w-5">
            {icons[txnFailed ? TransactionAction.Failed : variant]}
        </div>
    );
}
