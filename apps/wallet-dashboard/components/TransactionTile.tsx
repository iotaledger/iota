// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import React from 'react';
import TransactionIcon from './TransactionIcon';
import formatTimestamp from '@/lib/utils/time';
import { usePopups } from '@/hooks';
import { TransactionDetailsPopup, Button } from '@/components';
import { ExtendedTransaction } from '@/lib/interfaces';

interface TransactionTileProps {
    transaction: ExtendedTransaction;
}

function TransactionTile({ transaction }: TransactionTileProps): JSX.Element {
    const { openPopup, closePopup } = usePopups();

    const handleDetailsClick = () => {
        openPopup(<TransactionDetailsPopup transaction={transaction} onClose={closePopup} />);
    };

    return (
        <div className="border-gray-45 flex h-full w-full flex-row items-center space-x-4 rounded-md border border-solid p-4">
            <TransactionIcon state={transaction.state} action={transaction.action} />
            <div className="flex h-full w-full flex-col space-y-2">
                <h2>{transaction.action}</h2>
                {transaction?.timestamp && <span>{formatTimestamp(transaction.timestamp)}</span>}
            </div>
            <Button onClick={handleDetailsClick}>Details</Button>
        </div>
    );
}

export default TransactionTile;
