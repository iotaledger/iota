// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { DisplayFieldsResponse, IotaObjectResponse } from '@iota/iota-sdk/client';

import { hasDisplayData } from '../hasDisplayData';

export function getObjectDisplayLookup(objects: IotaObjectResponse[] = []) {
    const lookup: Map<string, DisplayFieldsResponse> = new Map();
    return objects?.filter(hasDisplayData).reduce((acc, curr) => {
        if (curr.data?.objectId) {
            acc.set(curr.data.objectId, curr.data.display as DisplayFieldsResponse);
        }
        return acc;
    }, lookup);
}
