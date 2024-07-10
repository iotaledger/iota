// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search } from '@iota/icons';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { AccountBalanceItem } from '_src/ui/app/components/accounts/AccountBalanceItem';
import { useAccountsFinder } from '_src/ui/app/hooks/useAccountsFinder';
import { useParams } from 'react-router-dom';

export function AccountsFinderView(): JSX.Element {
    const { accountSourceId } = useParams();
    const {
        data: finderAddresses,
        searchMore,
        init,
    } = useAccountsFinder({
        accountGapLimit: 10,
        addressGapLimit: 2,
        sourceID: accountSourceId || '',
    });

    return (
        <div className="flex h-full flex-1 flex-col justify-between">
            <div className="flex h-96 flex-col gap-4 overflow-y-auto">
                {finderAddresses?.map((finderAddress) => {
                    return (
                        <AccountBalanceItem
                            key={finderAddress.pubKeyHash}
                            finderAddress={finderAddress}
                        />
                    );
                })}
            </div>
            <div className="flex flex-col gap-2">
                <Button variant="outline" size="tall" text={'Start again'} onClick={init} />
                <Button
                    variant="outline"
                    size="tall"
                    text={finderAddresses?.length == 0 ? 'Search' : 'Search again'}
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
