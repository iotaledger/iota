// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ObjectChangesByOwner, IotaObjectChangeTypes } from '@iota/core';
import { ObjectDetail } from './';

interface ObjectChangeEntryProps {
    changes: ObjectChangesByOwner;
    type: IotaObjectChangeTypes;
}

export default function ObjectChangeEntry({ changes, type }: ObjectChangeEntryProps) {
    return (
        <>
            {Object.entries(changes).map(([owner, changes]) => (
                <div key={`${type}-${owner}`}>
                    <div>
                        {changes.changes.map((change, i) => (
                            <ObjectDetail ownerKey={owner} key={i} change={change} />
                        ))}
                    </div>
                </div>
            ))}
        </>
    );
}
