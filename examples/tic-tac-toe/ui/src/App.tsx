// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import './App.css';

import { ConnectButton, useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';
import { isValidIotaObjectId, normalizeIotaObjectId } from '@iota/iota-sdk/utils';
import { FrameIcon } from '@radix-ui/react-icons';
import { Box, Container, Flex, Heading, Link } from '@radix-ui/themes';
import { Error } from 'components/Error';
import { networkConfig, useNetworkVariable } from 'config';
import Game from 'pages/Game';
import Root from 'pages/Root';

function App() {
	// Ensure the app's network config matches the wallet's available networks, if the wallet is connected.
	const account = useCurrentAccount();
	const ctx = useIotaClientContext();

	const chain = account?.chains?.find((c) => c.startsWith('iota:'))?.replace(/^iota:/, '');
	if (chain) {
		console.debug('Configuring app for', chain);
		ctx.selectNetwork(chain);
	}

	return (
		<>
			<Flex position="sticky" px="4" py="2" align="center" justify="between">
				<Flex align="center" gap="1">
					<FrameIcon width={20} height={20} />
					<Heading>
						<Link href="/" className="home">
							Tic Tac Toe
						</Link>
					</Heading>
				</Flex>

				<Box>
					<ConnectButton />
				</Box>
			</Flex>
			<Container size="1" mt="8">
				<Content />
			</Container>
		</>
	);
}

function Content() {
	const packageId = useNetworkVariable('packageId');

	const path = location.pathname.slice(1);
	const addr = normalizeIotaObjectId(path);

	if (packageId === null) {
		const availableNetworks = Object.keys(networkConfig).filter(
			(n) => (networkConfig as any)[n]?.variables?.packageId,
		);

		return (
			<Error title="App not available">
				This app is only available on {availableNetworks.join(', ')}. Please switch your wallet to a
				supported network.
			</Error>
		);
	} else if (path === '') {
		return <Root />;
	} else if (isValidIotaObjectId(addr)) {
		return <Game id={addr} />;
	} else {
		return (
			<Error title="Invalid Game ID">
				<code>"{path}"</code> is not a valid IOTA object ID.
			</Error>
		);
	}
}

export default App;
