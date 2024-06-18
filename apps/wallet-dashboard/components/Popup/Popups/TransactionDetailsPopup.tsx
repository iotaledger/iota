// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import formatTimestamp from '@/lib/utils/time';
import { ExtendedTransaction } from '@/lib/interfaces';

interface TransactionDetailsPopupProps {
    transaction: ExtendedTransaction;
    onClose: () => void;
}

function TransactionDetailsPopup({
    transaction,
    onClose,
}: TransactionDetailsPopupProps): JSX.Element {
    return (
        <div className="flex w-full min-w-[300px] flex-col gap-2">
            <h2>Transaction Details</h2>
            <p>Action: {transaction.action}</p>
            <p>State: {transaction.state}</p>
            {transaction.timestamp && <p>Timestamp: {formatTimestamp(transaction.timestamp)}</p>}
        </div>
    );
}

export default TransactionDetailsPopup;
