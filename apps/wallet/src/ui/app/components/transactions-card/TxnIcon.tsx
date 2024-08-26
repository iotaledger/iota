// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator } from '_components';
import { ArrowBottomLeft, ArrowTopRight, Info, IotaLogoMark, Person, Stake } from '@iota/ui-icons';

const ICON_COLORS = {
    primary: 'text-primary-30',
    error: 'text-error-30',
};

const icons = {
    Send: <ArrowTopRight className={ICON_COLORS.primary} />,
    Receive: <ArrowBottomLeft className={ICON_COLORS.primary} />,
    Transaction: <ArrowTopRight className={ICON_COLORS.primary} />,
    Staked: <Stake className={ICON_COLORS.primary} />,
    Unstaked: <Stake className={ICON_COLORS.primary} />,
    Rewards: <IotaLogoMark className={ICON_COLORS.primary} />,
    Failed: <Info className={ICON_COLORS.error} />,
    Loading: <LoadingIndicator />,
    PersonalMessage: <Person className={ICON_COLORS.primary} />,
};

interface TxnItemIconProps {
    txnFailed?: boolean;
    variant: keyof typeof icons;
}

export function TxnIcon({ txnFailed, variant }: TxnItemIconProps) {
    return <div className="[&_svg]:h-5 [&_svg]:w-5">{icons[txnFailed ? 'Failed' : variant]}</div>;
}
