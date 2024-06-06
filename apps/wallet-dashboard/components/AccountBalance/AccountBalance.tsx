// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';

import { useBalance } from '@/hooks';

export const AccountBalance = () => {
    const account = useCurrentAccount();

    const { data, isLoading } = useBalance(IOTA_TYPE_ARG, account?.address);

    return (
        <div>
            {isLoading && <p>Loading...</p>}
            {!isLoading && <p>Balance: {data?.iotaBalance}</p>}
        </div>
    );
};
