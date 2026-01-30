// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext } from '@iota/dapp-kit';

export function useNetwork(): string {
    const iotaClientContext = useIotaClientContext();
    return iotaClientContext.network;
}
