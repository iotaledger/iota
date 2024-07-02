// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search24 } from '@iota/icons';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { useState } from 'react';
import { AccountBalanceItem } from '_src/ui/app/components/accounts/AccountBalanceItem';

interface AccountsFinderViewProps {
    accounts: {
        id: string;
        address: string;
        balance: number;
    }[];
}

export function AccountsFinderView({ accounts }: AccountsFinderViewProps): JSX.Element {
    const [isSearchExecuted, setIsSearchExecuted] = useState(false);

    function handleSearch(): void {
        // TODO: Implement search logic
        setIsSearchExecuted(true);
    }

    return (
        <div className="flex h-full flex-1 flex-col justify-between">
            <div className="flex h-96 flex-col gap-4 overflow-y-auto">
                {accounts.map((account) => {
                    return <AccountBalanceItem key={account.id} {...account} />;
                })}
            </div>
            <div className="flex flex-col gap-2">
                <Button
                    variant="outline"
                    size="tall"
                    text={!isSearchExecuted ? 'Search' : 'Search again'}
                    after={<Search24 />}
                    onClick={handleSearch}
                />

                <div className="flex flex-row gap-2">
                    <Button variant="outline" size="tall" text="Skip" />
                    <Button variant="outline" size="tall" text="Continue" />
                </div>
            </div>
        </div>
    );
}
