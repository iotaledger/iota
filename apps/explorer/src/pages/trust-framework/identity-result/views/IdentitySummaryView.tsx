// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, TooltipPosition } from '@iota/apps-ui-kit';
import { formatDate, useFormatCoin } from '@iota/core';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { CoinFormat, formatDigest } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import { ObjectLink, TransactionLink } from '~/components/ui';
import { onCopySuccess } from '~/lib/utils';
import { type IotaDocument } from '@iota/identity-wasm/web';
import { ErrorBoundary } from '~/components';

interface IdentitySummaryViewProps {
    didDocument: IotaDocument;
    objectData: IotaObjectData;
}

export function IdentitySummaryView({
    didDocument,
    objectData: { objectId, storageRebate, previousTransaction },
}: IdentitySummaryViewProps): JSX.Element {
    const isActive = didDocument.metadataDeactivated() !== true;

    const didDateFormat = (timestamp: string): string =>
        formatDate(new Date(timestamp), ['year', 'month', 'day', 'hour', 'minute']);
    const createdAt = didDateFormat(didDocument.metadataCreated()!.toRFC3339());
    const updatedAt = didDateFormat(didDocument.metadataUpdated()!.toRFC3339());

    return (
        <ErrorBoundary>
            <div className="flex flex-col gap-md">
                <div className={clsx('address-grid-container-top', 'no-image', 'no-description')}>
                    {objectId && (
                        <div>
                            <ObjectIdCard objectId={objectId} />
                        </div>
                    )}

                    <div>
                        <DisplayStats
                            label="Active"
                            value={isActive ? 'Yes' : 'No'}
                            tooltipPosition={TooltipPosition.Left}
                            tooltipText="Whether this Identity is currently active on the ledger. Once deleted, an Identity is permanently deactivated and cannot be recovered."
                        />
                    </div>

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
                                tooltipText="Timestamp of the transaction that first published this Identity onchain."
                            />
                        </div>
                    )}

                    {updatedAt && (
                        <div>
                            <DisplayStats
                                label="Updated at"
                                value={updatedAt}
                                tooltipPosition={TooltipPosition.Left}
                                tooltipText="Timestamp of the most recent transaction that modified this Identity. Any change to keys, services, or document content triggers an update."
                            />
                        </div>
                    )}
                    {previousTransaction && (
                        <div>
                            <LastTxBlockCard digest={previousTransaction} />
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
            tooltipText="The unique onchain identifier of the Move object storing this Identity's state. The Identity itself is derived from this Object ID."
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
            tooltipText="Hash of the most recent transaction that modified this Identity. Use it to inspect transaction details on the explorer."
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
