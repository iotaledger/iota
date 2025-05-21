// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PaginatedObjectsResponse } from '@iota/iota-sdk/client';
import { SUPPLY_INCREASE_VESTING_LABEL } from '../../constants';

export function haveSupplyIncreaseLabel(pages: PaginatedObjectsResponse[]) {
    return pages[pages.length - 1]?.data.some(
        (object) =>
            object.data?.content?.dataType === 'moveObject' &&
            object.data?.content?.fields &&
            'label' in object.data.content.fields &&
            object.data?.content?.fields?.label === SUPPLY_INCREASE_VESTING_LABEL,
    );
}
