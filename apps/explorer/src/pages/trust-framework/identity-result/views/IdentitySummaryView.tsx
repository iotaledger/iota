// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, TooltipPosition } from '@iota/apps-ui-kit';
import { useFormatCoin } from '@iota/core';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { CoinFormat, formatDigest } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import { DateDisplay, ErrorBoundary } from '~/components';
import { ObjectLink, TransactionLink } from '~/components/ui';
import { onCopySuccess } from '~/lib/utils';
import { type IotaDocument } from '@iota/identity-wasm/web';

interface IdentitySummaryViewProps {
    didDocument: IotaDocument;
    objectData: IotaObjectData;
}

export function IdentitySummaryView({
    didDocument,
    objectData: { objectId, storageRebate, previousTransaction },
}: IdentitySummaryViewProps): JSX.Element {
    const isActive = didDocument.metadataDeactivated() !== true;

    const createdAtMs = didDocument.metadataCreated()
        ? new Date(didDocument.metadataCreated()!.toRFC3339()).getTime()
        : null;
    const updatedAtMs = didDocument.metadataUpdated()
        ? new Date(didDocument.metadataUpdated()!.toRFC3339()).getTime()
        : null;

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

                    {createdAtMs && (
                        <div>
                            <DisplayStats
                                label="Created at"
                                value={<DateDisplay timestamp={createdAtMs} />}
                                tooltipPosition={TooltipPosition.Left}
                                tooltipText="Timestamp of the transaction that first published this Identity onchain."
                            />
                        </div>
                    )}

                    {updatedAtMs && (
                        <div>
                            <DisplayStats
                                label="Updated at"
                                value={<DateDisplay timestamp={updatedAtMs} />}
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
