// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCurrentAccount } from '@mysten/dapp-kit';
import { SUI_TYPE_ARG } from '@mysten/sui.js/utils';

import { useBalance } from '@/hooks';

export const AccountBalance = () => {
    const account = useCurrentAccount();

    const { data, isLoading } = useBalance(SUI_TYPE_ARG, account?.address);

    return (
        <div>
            {isLoading && <p>Loading...</p>}
            {!isLoading && <p>Balance: {data?.suiBalance}</p>}
        </div>
    );
};
