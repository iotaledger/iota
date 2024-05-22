// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { ConnectButton, useCurrentWallet } from '@mysten/dapp-kit';

export default function Home() {
	const { connectionStatus } = useCurrentWallet();

	return (
		<main className="flex min-h-screen flex-col items-center justify-between p-24">
			<ConnectButton />
			{connectionStatus === 'connected' ? (
				<div className="flex flex-col items-center justify-center">
					<h1>Welcome</h1>
					<p>Connection status: {connectionStatus}</p>
				</div>
			) : (
				<div>Connection status: {connectionStatus}</div>
			)}
		</main>
	);
}
