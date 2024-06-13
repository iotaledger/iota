// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ObjectChangeSummary, IotaObjectChangeTypes } from '@iota/core';
import { ObjectChangeEntry } from './';

interface ObjectChangesProps {
    objectSummary?: ObjectChangeSummary | null;
}

export default function ObjectChanges({ objectSummary }: ObjectChangesProps) {
    if (!objectSummary) return null;

    return (
        <>
            {Object.entries(objectSummary).map(([type, changes]) => (
                <ObjectChangeEntry
                    key={type}
                    type={type as IotaObjectChangeTypes}
                    changes={changes}
                />
            ))}
        </>
    );
}
