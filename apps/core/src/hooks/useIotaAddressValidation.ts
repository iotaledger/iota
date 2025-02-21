// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useMemo } from 'react';
import { createIotaAddressValidation } from '../utils';

export function useIotaAddressValidation() {
    return useMemo(() => {
        return createIotaAddressValidation();
    }, []);
}
