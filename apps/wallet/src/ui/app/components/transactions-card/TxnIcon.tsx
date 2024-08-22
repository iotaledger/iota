// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator } from '_components';
import { ArrowBottomLeft, ArrowTopRight, Info, IotaLogoMark, Person, Stake } from '@iota/ui-icons';

const icons = {
    Send: <ArrowTopRight className="text-primary-30" />,
    Receive: <ArrowBottomLeft className="text-primary-30" />,
    Transaction: <ArrowTopRight className="text-primary-30" />,
    Staked: <Stake className=" text-primary-30" />,
    Unstaked: <Stake className="text-primary-30" />,
    Rewards: <IotaLogoMark className="text-primary-30" />,
    Failed: <Info className="text-error-30" />,
    Loading: <LoadingIndicator />,
    PersonalMessage: <Person className="text-primary-30" />,
};

interface TxnItemIconProps {
    txnFailed?: boolean;
    variant: keyof typeof icons;
}

export function TxnIcon({ txnFailed, variant }: TxnItemIconProps) {
    return <div className="[&_svg]:h-5 [&_svg]:w-5">{icons[txnFailed ? 'Failed' : variant]}</div>;
}
