// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { clsx } from 'clsx';
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
    primary: 'text-iota-primary-30 dark:text-iota-primary-80',
    error: 'text-iota-error-30 dark:text-iota-error-80',
};

const icons = {
    [TransactionAction.Send]: <ArrowTopRight />,
    [TransactionAction.Receive]: <ArrowBottomLeft />,
    [TransactionAction.Transaction]: <ArrowTopRight />,
    [TransactionAction.Staked]: <Stake />,
    [TransactionAction.Unstaked]: <Unstake />,
    [TransactionAction.PersonalMessage]: <Person />,
    [TransactionAction.TimelockedStaked]: <Stake />,
    [TransactionAction.TimelockedUnstaked]: <Unstake />,
    [TransactionAction.Migration]: <Migration />,
    [TransactionAction.TimelockedCollect]: <Vesting />,
};

interface TransactionIconProps {
    txnFailed?: boolean;
    variant: TransactionAction;
}

export function TransactionIcon({ txnFailed, variant }: TransactionIconProps) {
    return (
        <div
            className={clsx(
                '[&_svg]:h-5 [&_svg]:w-5',
                txnFailed ? ICON_COLORS.error : ICON_COLORS.primary,
            )}
        >
            {txnFailed ? <Info /> : icons[variant]}
        </div>
    );
}
