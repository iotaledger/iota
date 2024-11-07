// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    IotaObjectChangeWithDisplay,
    getObjectChangeLabel,
    getOwnerDisplay,
    parseObjectChangeDetails,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { DisplayFieldsResponse, IotaObjectChange } from '@iota/iota-sdk/client';
import { formatAddress, isValidIotaAddress } from '@iota/iota-sdk/utils';

enum ObjectDetailLabel {
    Package = 'Package',
    Module = 'Module',
    Type = 'Type',
}

interface ObjectDetailProps {
    change: IotaObjectChange | IotaObjectChangeWithDisplay;
    owner: string;
    ownerType: string;
    displayData?: DisplayFieldsResponse;
}

export default function ObjectDetail({ change, owner, ownerType, displayData }: ObjectDetailProps) {
    const address = useCurrentAccount()?.address;
    if (!address) return null;

    const { ownerDisplay } = getOwnerDisplay(owner, ownerType, address);

    if (change.type === 'transferred' || change.type === 'published') {
        return null;
    }

    const [packageId, moduleName, typeName] = parseObjectChangeDetails(change);

    const objectDetails: {
        label: ObjectDetailLabel;
        value: string;
    }[] = [
        {
            label: ObjectDetailLabel.Package,
            value: packageId,
        },
        {
            label: ObjectDetailLabel.Module,
            value: moduleName,
        },
        {
            label: ObjectDetailLabel.Type,
            value: typeName,
        },
    ];

    return (
        <div className="py-2">
            <h4 className="text-center font-semibold">{getObjectChangeLabel(change.type)}</h4>

            <div className="flex flex-row items-center justify-between py-2">
                <div className="flex flex-col">
                    {change.objectId && (
                        <div className="flex flex-row space-x-2">
                            <span className="font-semibold">Object</span>
                            <div title={change.objectId}>{formatAddress(change.objectId)}</div>
                        </div>
                    )}

                    {objectDetails.map((item) => {
                        const shouldFormatValue = isValidIotaAddress(item.value);
                        return (
                            <div key={item.label} className="flex flex-row space-x-2">
                                <span className="font-semibold">{item.label}</span>
                                <div title={shouldFormatValue ? item.value : undefined}>
                                    {shouldFormatValue ? formatAddress(item.value) : item.value}
                                </div>
                            </div>
                        );
                    })}
                </div>

                {displayData && displayData.data && (
                    // eslint-disable-next-line @next/next/no-img-element
                    <img
                        src={displayData.data.image_url}
                        alt={displayData.data.name}
                        height={100}
                        width={100}
                    />
                )}
            </div>

            {ownerDisplay && (
                <div className="flex flex-row justify-between space-x-2 border-t pt-1">
                    <span>Owner</span>
                    <span title={owner}>{ownerDisplay}</span>
                </div>
            )}
        </div>
    );
}
