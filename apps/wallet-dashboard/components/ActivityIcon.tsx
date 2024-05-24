// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
	Account24,
	ArrowRight16,
	Info16,
	Sui,
	Swap16,
	Unstaked,
	WalletActionStake24,
} from '@mysten/icons';

const icons = {
	Send: (
		<ArrowRight16 fill="currentColor" className="text-gradient-blue-start text-body -rotate-45" />
	),
	Receive: (
		<ArrowRight16 fill="currentColor" className="text-gradient-blue-start text-body rotate-135" />
	),
	Transaction: (
		<ArrowRight16 fill="currentColor" className="text-gradient-blue-start text-body -rotate-45" />
	),
	Staked: <WalletActionStake24 className="text-gradient-blue-start text-heading2 bg-transparent" />,
	Unstaked: <Unstaked className="text-gradient-blue-start text-heading3" />,
	Rewards: <Sui className="text-gradient-blue-start text-body" />,
	Swapped: <Swap16 className="text-gradient-blue-start text-heading6" />,
	Failed: <Info16 className="text-issue-dark text-heading6" />,
	PersonalMessage: <Account24 fill="currentColor" className="text-gradient-blue-start text-body" />,
};

interface ActivityIconProps {
	transactionFailed?: boolean;
	action: keyof typeof icons;
}

function ActivityIcon({ transactionFailed, action }: ActivityIconProps) {
	return (
		<div className={`${transactionFailed ? 'bg-issue-light' : 'bg-gray-40'} w-7.5 h-7.5 flex justify-center items-center rounded-2lg`}>
			{icons[transactionFailed ? 'Failed' : action]}
		</div>
	);
}

export default ActivityIcon;