// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, TooltipPosition } from '@iota/apps-ui-kit';
import { formatDate, useFormatCoin } from '@iota/core';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { CoinFormat, formatDigest } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import { ObjectLink, TransactionLink } from '~/components/ui';
import { onCopySuccess } from '~/lib/utils';
import { ErrorBoundary } from '~/components';
import { type OnChainNotarization } from '@iota/notarization/web';

interface NotarizationSummaryViewProps {
    notarizationDocument: OnChainNotarization;
    objectData: IotaObjectData;
}

export function NotarizastionSummaryView({
    notarizationDocument,
    objectData,
}: NotarizationSummaryViewProps): JSX.Element {
    const objectId = objectData?.objectId;
    const storageRebate = objectData?.storageRebate;
    const versionCount = `v${notarizationDocument.stateVersionCount}`;

    const dateFormat = (timestamp: bigint): string =>
        // Convert seconds to milliseconds for Date constructor
        formatDate(new Date(Number(timestamp)), ['year', 'month', 'day', 'hour', 'minute']);
    const createdAt = dateFormat(notarizationDocument.immutableMetadata.createdAt);
    const updatedAt = dateFormat(notarizationDocument.lastStateChangeAt);
    const lastTransactionBlockDigest = objectData?.previousTransaction;

    return (
        <ErrorBoundary>
            <div className="flex flex-col gap-md">
                <div className={clsx('address-grid-container-top', 'no-image', 'no-description')}>
                    {objectId && (
                        <div>
                            <ObjectIdCard objectId={objectId} />
                        </div>
                    )}

                    {versionCount && (
                        <div>
                            <DisplayStats
                                label="Version"
                                value={versionCount}
                                tooltipPosition={TooltipPosition.Left}
                                tooltipText="Version of state change in a progressive sequence."
                            />
                        </div>
                    )}

                    {storageRebate && (
                        <div>
                            <StorageRebateCard storageRebate={storageRebate} />
                        </div>
                    )}

                    {createdAt && (
                        <div>
                            <DisplayStats
                                label="Created at"
                                value={createdAt}
                                tooltipPosition={TooltipPosition.Left}
                                tooltipText="Timestamp of the transaction that first published this notarization onchain."
                            />
                        </div>
                    )}

                    {updatedAt && (
                        <div>
                            <DisplayStats
                                label="Updated at"
                                value={updatedAt}
                                tooltipPosition={TooltipPosition.Left}
                                tooltipText="Timestamp of the most recent transaction that modified this notarization. The version badge shows the current state version count."
                            />
                        </div>
                    )}

                    {lastTransactionBlockDigest && (
                        <div>
                            <LastTxBlockCard digest={lastTransactionBlockDigest} />
                        </div>
                    )}
                </div>
            </div>
        </ErrorBoundary>
    );
}

interface ObjectIdCardProps {
    objectId: string;
}

function ObjectIdCard({ objectId }: ObjectIdCardProps): JSX.Element {
    return (
        <DisplayStats
            label="Object ID"
            value={
                <div className="flex flex-col gap-xs">
                    <ObjectLink objectId={objectId} copyText={objectId} />
                </div>
            }
            tooltipPosition={TooltipPosition.Left}
            tooltipText="The unique onchain identifier of the Move object storing this notarization's state."
        />
    );
}

interface LastTxBlockCardProps {
    digest: string;
}

function LastTxBlockCard({ digest }: LastTxBlockCardProps): JSX.Element {
    return (
        <DisplayStats
            label="Last Transaction Block Digest"
            value={<TransactionLink digest={digest}>{formatDigest(digest)}</TransactionLink>}
            copyText={digest}
            onCopySuccess={onCopySuccess}
            tooltipPosition={TooltipPosition.Left}
            tooltipText="Hash of the most recent transaction that modified this notarization. Use it to inspect transaction details on the explorer."
        />
    );
}

interface StorageRebateCardProps {
    storageRebate: string;
}

function StorageRebateCard({ storageRebate }: StorageRebateCardProps): JSX.Element | null {
    const [storageRebateFormatted, symbol] = useFormatCoin({
        balance: storageRebate,
        format: CoinFormat.Full,
    });

    return (
        <DisplayStats
            label="Storage Rebate"
            value={`-${storageRebateFormatted}`}
            supportingLabel={symbol}
            tooltipPosition={TooltipPosition.Left}
            tooltipText="IOTA tokens locked as a storage deposit for this object. Partially refundable when the object is deleted or reduced in size."
        />
    );
}
