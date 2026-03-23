// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Badge,
    BadgeType,
    Button,
    ButtonSize,
    ButtonType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    KeyValueInfo,
    LoadingIndicator,
    Title,
    TitleSize,
    TooltipPosition,
} from '@iota/apps-ui-kit';
import { formatDate } from '@iota/core';
import { formatDigest } from '@iota/iota-sdk/utils';
import { Warning, Person } from '@iota/apps-ui-icons';
import {
    AddressLink,
    CollapsibleCard,
    ErrorBoundary,
    IconBadge,
    ObjectLink,
    TransactionLink,
} from '~/components';
import { useGetOwnerHistory, type OwnerEntry } from '../hooks/useGetOwnerHistory';

enum OwnerLabel {
    Current = 'Current',
    Previous = 'Previous',
}

interface OwnersViewProps {
    objectId: string;
}

export function OwnersView({ objectId }: OwnersViewProps): JSX.Element {
    const { owners, isPending, isError, hasNextPage, isFetchingNextPage, fetchNextPage } =
        useGetOwnerHistory(objectId);

    return (
        <ErrorBoundary>
            <div className="flex w-full flex-col gap-sm">
                <Title
                    title="Owners History"
                    tooltipPosition={TooltipPosition.Left}
                    tooltipText="The history of addresses that have owned this notarization object, ordered from most recent to oldest."
                />
                <div className="flex flex-col gap-sm">
                    {isPending && (
                        <div className="flex justify-center">
                            <LoadingIndicator size="w-6 h-6" text="Loading owners..." />
                        </div>
                    )}
                    {isError && (
                        <InfoBox
                            title="Error Fetching Owners"
                            supportingText={`Could not fetch owner history for object ${objectId} on the current network.`}
                            icon={<Warning />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
                    )}
                    {owners.map((owner, index) => (
                        <OwnerCard
                            key={owner.transactionDigest}
                            owner={owner}
                            label={index === 0 ? OwnerLabel.Current : OwnerLabel.Previous}
                        />
                    ))}
                    {hasNextPage && (
                        <div className="flex justify-center">
                            <Button
                                size={ButtonSize.Small}
                                type={ButtonType.Ghost}
                                text={isFetchingNextPage ? 'Loading...' : 'Identify More Owners'}
                                disabled={isFetchingNextPage}
                                onClick={() => fetchNextPage()}
                            />
                        </div>
                    )}
                </div>
            </div>
        </ErrorBoundary>
    );
}

interface OwnerCardProps {
    owner: OwnerEntry;
    label: OwnerLabel;
}

function OwnerCard({ owner, label }: OwnerCardProps): JSX.Element {
    const badgeType = label === OwnerLabel.Current ? BadgeType.PrimarySoft : BadgeType.Neutral;
    const timestamp = owner.timestampMs
        ? formatDate(new Date(Number(owner.timestampMs)), [
              'year',
              'month',
              'day',
              'hour',
              'minute',
          ])
        : null;

    return (
        <CollapsibleCard
            collapsible
            title="Owner"
            titleSize={TitleSize.Small}
            supportingTitleElement={
                <div className="ml-1 flex gap-x-1">
                    <Badge label={owner.ownerType} type={BadgeType.Neutral} />
                    <IconBadge label={label} type={badgeType} icon={<Person />} />
                </div>
            }
            footer={<OwnerCardFooter owner={owner} />}
        >
            <div className="flex flex-col gap-4 py-sm--rs">
                <OwnerType owner={owner} />
                <OwnershipTransactionLink owner={owner} />
                <TransactionDate timestamp={timestamp} />
            </div>
        </CollapsibleCard>
    );
}

function OwnerType({ owner }: { owner: OwnerEntry }) {
    return (
        <div className="flex flex-wrap px-md--rs">
            <KeyValueInfo
                keyText="Type"
                value={owner.ownerType}
                fullwidth
                tooltipPosition={TooltipPosition.Left}
                tooltipText="The ownership type: address-owned, object-owned, or shared."
            />
        </div>
    );
}

function OwnerAddress({ owner }: { owner: OwnerEntry }) {
    return (
        <div className="flex flex-wrap px-md--rs">
            <KeyValueInfo
                keyText="Owner"
                value={
                    <OwnerAddressDisplay
                        ownerType={owner.ownerType}
                        ownerAddress={owner.ownerAddress}
                    />
                }
                fullwidth
                tooltipPosition={TooltipPosition.Left}
                tooltipText="The address that owns or owned this notarization object."
            />
        </div>
    );
}

interface OwnerAddressDisplayProps {
    ownerType: string;
    ownerAddress: string;
}

function OwnerAddressDisplay({ ownerType, ownerAddress }: OwnerAddressDisplayProps): JSX.Element {
    if (ownerType === 'AddressOwner') {
        return (
            <AddressLink
                address={ownerAddress}
                copyText={ownerAddress}
                className="[&>div]:max-w-[200px] [&>div]:truncate"
                display="block"
            />
        );
    }

    if (ownerType === 'ObjectOwner') {
        return <ObjectLink objectId={ownerAddress} copyText={ownerAddress} />;
    }

    if (ownerType === 'Shared') {
        return <ObjectLink objectId={ownerAddress} label="Shared" showAddressAlias={false} />;
    }

    return <span>{ownerAddress}</span>;
}

interface OwnerCardFooterProps {
    owner: OwnerEntry;
}

function OwnerCardFooter({ owner }: OwnerCardFooterProps): JSX.Element {
    return (
        <div className="flex flex-col gap-4 py-sm--rs">
            <OwnerAddress owner={owner} />
        </div>
    );
}

function TransactionDate({ timestamp }: { timestamp: string | null }) {
    return (
        timestamp && (
            <div className="flex flex-wrap px-md--rs">
                <KeyValueInfo
                    keyText="Date"
                    value={timestamp}
                    fullwidth
                    tooltipPosition={TooltipPosition.Left}
                    tooltipText="When this ownership change occurred."
                />
            </div>
        )
    );
}

function OwnershipTransactionLink({ owner }: { owner: OwnerEntry }) {
    return (
        <div className="flex flex-wrap px-md--rs">
            <KeyValueInfo
                keyText="Transaction"
                value={
                    <TransactionLink digest={owner.transactionDigest}>
                        {formatDigest(owner.transactionDigest)}
                    </TransactionLink>
                }
                fullwidth
                tooltipPosition={TooltipPosition.Left}
                tooltipText="The transaction that assigned this owner to the notarization object."
            />
        </div>
    );
}
