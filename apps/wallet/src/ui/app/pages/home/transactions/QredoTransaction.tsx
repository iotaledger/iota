// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toUtf8OrB64 } from '_src/shared/utils';
import LoadingIndicator from '_src/ui/app/components/loading/LoadingIndicator';
import { TxnIcon } from '_src/ui/app/components/transactions-card/TxnIcon';
import { useGetQredoTransaction } from '_src/ui/app/hooks/useGetQredoTransaction';
import { Text } from '_src/ui/app/shared/text';
import { formatDate, useOnScreen } from '@iota/core';
import { bcs } from '@iota/iota-sdk/bcs';
import { fromBase64 } from '@iota/iota-sdk/utils';
import { useMemo, useRef } from 'react';

export type QredoTransactionProps = {
    qredoID?: string;
    qredoTransactionID?: string;
};

export function QredoTransaction({ qredoID, qredoTransactionID }: QredoTransactionProps) {
    const transactionElementRef = useRef<HTMLDivElement>(null);
    const { isIntersecting } = useOnScreen(transactionElementRef);
    const { data, isPending, error } = useGetQredoTransaction({
        qredoID,
        qredoTransactionID,
        forceDisabled: !isIntersecting,
    });
    const messageWithIntent = useMemo(() => {
        if (data?.MessageWithIntent) {
            return fromBase64(data.MessageWithIntent);
        }
        return null;
    }, [data?.MessageWithIntent]);

    const isSignMessage = messageWithIntent
        ? bcs.IntentScope.parse(messageWithIntent).PersonalMessage
        : false;

    const transactionBytes = useMemo(
        () => messageWithIntent?.slice(3) || null,
        [messageWithIntent],
    );
    const messageToSign =
        useMemo(
            () => transactionBytes && toUtf8OrB64(transactionBytes),
            [transactionBytes],
        )?.message?.slice(0, 300) || null;
    return (
        <div ref={transactionElementRef} className="flex items-start gap-3 py-4">
            <div>
                <TxnIcon
                    txnFailed={!!error}
                    variant={isPending ? 'Loading' : isSignMessage ? 'PersonalMessage' : 'Send'}
                />
            </div>
            <div className="flex flex-col gap-1 overflow-hidden">
                {isPending ? (
                    <>
                        <div className="h-3 w-20 rounded bg-iota-lightest" />
                        <div className="h-3 w-16 rounded bg-iota-lightest" />
                    </>
                ) : data ? (
                    <>
                        <div className="item-center flex flex-nowrap gap-1">
                            <Text color="gray-90" weight="semibold">
                                {isSignMessage ? 'Sign personal message' : 'Transaction'}
                            </Text>
                            <Text color="gray-90" variant="bodySmall">
                                ({data.status})
                            </Text>
                        </div>
                        <Text color="gray-80" mono variant="bodySmall">
                            #{data.txID}
                        </Text>
                        {isSignMessage && messageToSign ? (
                            <div className="line-clamp-3 overflow-hidden break-words">
                                <Text color="gray-80" weight="normal">
                                    {messageToSign}
                                </Text>
                            </div>
                        ) : null}
                        {data.timestamps.created ? (
                            <Text color="steel-dark" variant="subtitleSmallExtra" weight="medium">
                                {formatDate(data.timestamps.created * 1000, [
                                    'month',
                                    'day',
                                    'hour',
                                    'minute',
                                ])}
                            </Text>
                        ) : null}
                        <div className="flex items-center gap-1.5 text-issue">
                            <Text weight="medium" variant="pBodySmall">
                                Check status in Qredo app
                            </Text>
                            <LoadingIndicator color="inherit" />
                        </div>
                    </>
                ) : (
                    <Text color="gray-80">
                        {(error as Error)?.message ||
                            'Something went wrong while fetching transaction details'}
                    </Text>
                )}
            </div>
        </div>
    );
}
