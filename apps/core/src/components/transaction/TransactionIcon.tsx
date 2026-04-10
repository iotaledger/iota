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

export enum TransactionIconSize {
    Small = 'small',
    Medium = 'medium',
    Large = 'large',
}

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

const ICON_SIZES: Record<TransactionIconSize, string> = {
    [TransactionIconSize.Small]: '[&_svg]:size-4',
    [TransactionIconSize.Medium]: '[&_svg]:size-5',
    [TransactionIconSize.Large]: '[&_svg]:size-6',
};

interface TransactionIconProps {
    txnFailed?: boolean;
    variant: TransactionAction;
    size?: TransactionIconSize;
}

export function TransactionIcon({
    txnFailed,
    variant,
    size = TransactionIconSize.Medium,
}: TransactionIconProps) {
    return (
        <div
            className={clsx(ICON_SIZES[size], txnFailed ? ICON_COLORS.error : ICON_COLORS.primary)}
        >
            {txnFailed ? <Info /> : icons[variant]}
        </div>
    );
}
