// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaObjectChangeWithDisplay } from '@iota/core';
import { formatAddress } from '@iota/iota.js/utils';

enum ObjectDetailLabel {
    Package = 'Package',
    Module = 'Module',
    Type = 'Type',
}
interface ObjectDetailProps {
    change: IotaObjectChangeWithDisplay;
    ownerKey: string;
}

export default function ObjectDetail({ change }: ObjectDetailProps) {
    if (change.type === 'transferred' || change.type === 'published') {
        return null;
    }

    const [packageId, moduleName, typeName] = change.objectType?.split('<')[0]?.split('::') || [];

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
            <div className="flex space-x-2">
                <span className="font-semibold">Object</span>
                {change.objectId && <div>{formatAddress(change.objectId)}</div>}
            </div>
            <div>
                {objectDetails.map((item) => (
                    <div key={item.label} className="flex flex-row space-x-2">
                        <span className="font-semibold">{item.label}</span>
                        <div>{item.value}</div>
                    </div>
                ))}
            </div>
        </div>
    );
}
