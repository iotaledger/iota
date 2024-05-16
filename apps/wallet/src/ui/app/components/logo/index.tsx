// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { SuiCustomRpc, SuiDevnet, SuiLocal, SuiMainnet, SuiTestnet } from '@mysten/icons';
import { Network } from '@mysten/sui.js/client';

type LogoProps = {
	network?: Network;
};

const networkLogos = {
	[Network.Mainnet]: SuiMainnet,
	[Network.Devnet]: SuiDevnet,
	[Network.Testnet]: SuiTestnet,
	[Network.Local]: SuiLocal,
	[Network.Custom]: SuiCustomRpc,
};

const Logo = ({ network }: LogoProps) => {
	let LogoComponent = networkLogos[Network.Custom];

	if (network && networkLogos[network]) {
		LogoComponent = networkLogos[network];
	}

	return <LogoComponent className="h-7 w-walletLogo text-gray-90" />;
};

export default Logo;
