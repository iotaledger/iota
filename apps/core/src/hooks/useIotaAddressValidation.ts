// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaNSEnabled } from '.';
import { useIotaClient } from '@iota/dapp-kit';
import { useMemo } from 'react';
import { createIotaAddressValidation } from '../utils';

export function useIotaAddressValidation() {
    const client = useIotaClient();
    const iotaNSEnabled = useIotaNSEnabled();

    return useMemo(() => {
        return createIotaAddressValidation(client, iotaNSEnabled);
    }, [client, iotaNSEnabled]);
}
