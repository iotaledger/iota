// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ExplorerLink, ExplorerLinkType, NftImage } from '_components';
import { type IotaObjectChangeWithDisplay } from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';

import { Text } from '../../../text';

export function ObjectChangeDisplay({ change }: { change: IotaObjectChangeWithDisplay }) {
    const display = change?.display?.data;
    const objectId = 'objectId' in change && change?.objectId;

    if (!display) return null;
    return (
        <div className="group relative w-32 min-w-min cursor-pointer whitespace-nowrap">
            <NftImage title={display.name ?? ''} src={display.image_url ?? ''} />
            {objectId && (
                <div className="full absolute bottom-2 left-1/2 -translate-x-1/2 justify-center rounded-lg bg-white/90 px-2 py-1 opacity-0 transition-opacity group-hover:opacity-100">
                    <ExplorerLink
                        type={ExplorerLinkType.Object}
                        objectID={objectId}
                        className="text-hero-dark no-underline"
                    >
                        <Text variant="pBodySmall" truncate mono>
                            {formatAddress(objectId)}
                        </Text>
                    </ExplorerLink>
                </div>
            )}
        </div>
    );
}
