// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClient } from '@iota/dapp-kit';
import { useMemo } from 'react';
import { createIotaAddressValidation } from '../utils';

export function useIotaAddressValidation() {
    const client = useIotaClient();

    return useMemo(() => {
        return createIotaAddressValidation(client);
    }, [client]);
}
