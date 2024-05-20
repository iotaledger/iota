// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { getCustomNetwork } from '_src/shared/api-env';
import { getNetwork, Network } from '@mysten/sui.js/client';
import { useMemo } from 'react';

import {
	getAddressUrl,
	getObjectUrl,
	getTransactionUrl,
	getValidatorUrl,
} from '../components/explorer-link//Explorer';
import { ExplorerLinkType } from '../components/explorer-link/ExplorerLinkType';
import { useActiveAddress } from './useActiveAddress';
import useAppSelector from './useAppSelector';

export type ExplorerLinkConfig =
	| {
			type: ExplorerLinkType.address;
			address?: string;
			useActiveAddress?: false;
	  }
	| {
			type: ExplorerLinkType.address;
			useActiveAddress: true;
	  }
	| { type: ExplorerLinkType.object; objectID: string; moduleName?: string }
	| { type: ExplorerLinkType.transaction; transactionID: string }
	| { type: ExplorerLinkType.validator; validator: string };

function useAddress(linkConfig: ExplorerLinkConfig) {
	const { type } = linkConfig;
	const isAddress = type === ExplorerLinkType.address;
	const isProvidedAddress = isAddress && !linkConfig.useActiveAddress;
	const activeAddress = useActiveAddress();
	return isProvidedAddress ? linkConfig.address : activeAddress;
}

export function useExplorerLink(linkConfig: ExplorerLinkConfig) {
	const { type } = linkConfig;
	const address = useAddress(linkConfig);
	const network = useAppSelector(({ app }) => app.network);
	const objectID = type === ExplorerLinkType.object ? linkConfig.objectID : null;
	const transactionID = type === ExplorerLinkType.transaction ? linkConfig.transactionID : null;
	const validator = type === ExplorerLinkType.validator ? linkConfig.validator : null;
	const moduleName = type === ExplorerLinkType.object ? linkConfig.moduleName : null;

	// fallback to localhost if customRPC is not set
	const customExplorer =
		network === Network.Custom ? getCustomNetwork().explorer : getNetwork(network).explorer;
	return useMemo(() => {
		if (!address) return null;
		switch (type) {
			case ExplorerLinkType.address:
				return address && getAddressUrl(address, network, customExplorer);
			case ExplorerLinkType.object:
				return objectID && getObjectUrl(objectID, network, customExplorer, moduleName);
			case ExplorerLinkType.transaction:
				return transactionID && getTransactionUrl(transactionID, network, customExplorer);
			case ExplorerLinkType.validator:
				return validator && getValidatorUrl(validator, network, customExplorer);
		}
	}, [type, address, network, customExplorer, moduleName, objectID, transactionID, validator]);
}
