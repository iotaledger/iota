// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { ConnectButton, useCurrentAccount, useCurrentWallet } from '@mysten/dapp-kit';

export default function Home() {
	const { connectionStatus } = useCurrentWallet();
	const account = useCurrentAccount();

	return (
		<main className="flex min-h-screen flex-col items-center space-y-8 p-24">
			<ConnectButton />
			{connectionStatus === 'connected' && account ? (
				<div className="flex flex-col items-center justify-center space-y-2">
					<h1>Welcome</h1>
					<p>Connection status: {connectionStatus}</p>
					<div>Address: {account.address}</div>
				</div>
			) : (
				<div>Connection status: {connectionStatus}</div>
			)}
		</main>
	);
}
