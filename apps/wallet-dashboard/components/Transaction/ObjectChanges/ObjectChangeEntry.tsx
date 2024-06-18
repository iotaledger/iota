// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ObjectChangesByOwner, IotaObjectChangeTypes } from '@iota/core';
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
                    {changes.changes.map((change, i) => (
                        <ObjectDetail
                            owner={owner}
                            ownerType={changes.ownerType}
                            key={i}
                            change={change}
                        />
                    ))}
                    {changes.changesWithDisplay.map((change, i) => (
                        <ObjectDetail
                            owner={owner}
                            ownerType={changes.ownerType}
                            key={i}
                            change={change}
                            displayData={change.display}
                        />
                    ))}
                </div>
            ))}
        </>
    );
}
