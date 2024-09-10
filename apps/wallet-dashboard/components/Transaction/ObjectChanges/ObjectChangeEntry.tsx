// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ObjectChangesByOwner, IotaObjectChangeTypes } from '@iota/core';
import { IotaObjectChange } from '@iota/iota-sdk/client';
import { ObjectDetail } from './';
import React from 'react';

interface ObjectChangeEntryProps {
    changes: ObjectChangesByOwner;
    type: IotaObjectChangeTypes;
}

export default function ObjectChangeEntry({ changes, type }: ObjectChangeEntryProps) {
    return (
        <>
            {Object.entries(changes).map(([owner, changes]) => (
                <div className="flex flex-col space-y-2 divide-y" key={`${type}-${owner}`}>
                    {changes.changes.map((change) => (
                        <ObjectDetail
                            owner={owner}
                            ownerType={changes.ownerType}
                            key={getChangeKey(change)}
                            change={change}
                        />
                    ))}
                    {changes.changesWithDisplay.map((change) => (
                        <ObjectDetail
                            owner={owner}
                            ownerType={changes.ownerType}
                            key={getChangeKey(change)}
                            change={change}
                            displayData={change.display}
                        />
                    ))}
                </div>
            ))}
        </>
    );
}

function getChangeKey(change: IotaObjectChange) {
    return change.type !== 'deleted' && change.type !== 'wrapped' ? change.digest : change.objectId;
}
