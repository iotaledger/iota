// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats } from '@iota/apps-ui-kit';
import { formatDate, useFormatCoin } from '@iota/core';
import { type IotaObjectData } from '@iota/iota-sdk/client';
import { CoinFormat, formatDigest } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import { ObjectLink, TransactionLink } from '~/components/ui';
import { onCopySuccess } from '~/lib/utils';
import { type IotaDocument } from '@iota/identity-wasm/web';
import { ErrorBoundary } from '~/components';

interface DidSummaryViewProps {
    didDocument: IotaDocument;
    objectData: IotaObjectData;
}

export function DidSummaryView({
    didDocument,
    objectData: { objectId, storageRebate, previousTransaction },
}: DidSummaryViewProps): JSX.Element {
    const isActive = didDocument.metadataDeactivated() !== true;

    const didDateFormat = (timestamp: string): string =>
        formatDate(new Date(timestamp), ['year', 'month', 'day', 'hour', 'minute']);
    const createdAt = didDateFormat(didDocument.metadataUpdated()!.toRFC3339());
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
                        <DisplayStats label="Active" value={isActive ? 'Yes' : 'No'} />
                    </div>

                    {storageRebate && (
                        <div>
                            <StorageRebateCard storageRebate={storageRebate} />
                        </div>
                    )}

                    {createdAt && (
                        <div>
                            <DisplayStats label="Created at" value={createdAt} />
                        </div>
                    )}

                    {updatedAt && (
                        <div>
                            <DisplayStats label="Updated at" value={updatedAt} />
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
        />
    );
}
