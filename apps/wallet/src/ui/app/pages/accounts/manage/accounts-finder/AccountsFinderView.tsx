// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search } from '@iota/icons';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { AccountBalanceItem } from '_src/ui/app/components/accounts/AccountBalanceItem';
import { useAccountsFinder } from '_src/ui/app/hooks/useAccountsFinder';
import { useParams } from 'react-router-dom';
import { useActiveAccount } from '_app/hooks/useActiveAccount';
import { type AllowedAccountTypes } from '_src/background/accounts-finder';
import { useAccounts } from '_src/ui/app/hooks/useAccounts';
import { getKey } from '_src/ui/app/helpers/accounts';
import { useState } from 'react';

export function AccountsFinderView(): JSX.Element {
    const { accountSourceId } = useParams();
    const { data: accounts } = useAccounts();
    const persistedAccounts = accounts?.filter((acc) => getKey(acc) === accountSourceId);
    const currentAccount = useActiveAccount();
    const [searched, setSearched] = useState(false);

    const { search, reset } = useAccountsFinder({
        accountType: currentAccount?.type as AllowedAccountTypes,
        sourceID: accountSourceId || '',
    });

    function searchMore() {
        setSearched(true);
        search();
    }

    return (
        <div className="flex h-full flex-1 flex-col justify-between">
            <div className="flex h-96 flex-col gap-4 overflow-y-auto">
                {persistedAccounts?.map((account) => {
                    return <AccountBalanceItem key={account.id} account={account} />;
                })}
            </div>
            <div className="flex flex-col gap-2">
                <Button variant="outline" size="tall" text={'Start again'} onClick={reset} />
                <Button
                    variant="outline"
                    size="tall"
                    text={searched ? 'Search again' : 'Search'}
                    after={<Search />}
                    onClick={searchMore}
                />

                <div className="flex flex-row gap-2">
                    <Button variant="outline" size="tall" text="Skip" />
                    <Button variant="outline" size="tall" text="Continue" />
                </div>
            </div>
        </div>
    );
}
