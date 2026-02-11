// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Badge,
    BadgeType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    KeyValueInfo,
    LoadingIndicator,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import { AddressLink, CollapsibleCard, ObjectLink } from '~/components';
import { type IotaObjectData } from '@iota/iota-sdk/src/client';
import { ObjectDetail } from '~/components/object/ObjectDetail';
import { Warning } from '@iota/apps-ui-icons';
import { useGetControllerObjects } from '../hooks/useGetControllerObjects';
import { extractThreshold } from '../helper';

interface ControllerViewProps {
    objectData: IotaObjectData;
}

export function ControllerView({ objectData }: ControllerViewProps) {
    const { controllers, isPending, isError } = useGetControllerObjects(objectData);
    const threshold = extractThreshold(objectData);

    return (
        <div className="flex w-full flex-col gap-sm">
            <Title title="Controller" />
            <div className="flex flex-col">
                {isPending && (
                    <div className="flex justify-center">
                        <LoadingIndicator size="w-6 h-6" text="Loading controllers..." />
                    </div>
                )}
                {isError && (
                    <InfoBox
                        title="Error Fetching DID's Controllers"
                        supportingText={`Could not fetch controllers of DID object ${objectData.objectId} on the current network.`}
                        icon={<Warning />}
                        type={InfoBoxType.Error}
                        style={InfoBoxStyle.Elevated}
                    />
                )}
                {controllers.map((controller) => (
                    <>
                        {controller.isError ? (
                            <InfoBox
                                key={controller.objectId}
                                title="Error Fetching Controller"
                                supportingText={`Could not fetch ControllerCap ${controller.objectId} on the current network.`}
                                icon={<Warning />}
                                type={InfoBoxType.Error}
                                style={InfoBoxStyle.Elevated}
                            />
                        ) : (
                            <CollapsibleCard
                                key={controller.objectId}
                                collapsible
                                title="Controller Capability"
                                titleSize={TitleSize.Small}
                                footer={
                                    // NOTE: How to test a shared owner?
                                    <ControllerCardFooter
                                        weight={controller.weight}
                                        threshold={threshold}
                                        ownerType={controller.ownerType!}
                                        ownerAddress={controller.owner!}
                                    />
                                }
                                supportingTitleElement={
                                    <div className="ml-1 flex">
                                        <Badge
                                            label={controller.ownerType}
                                            type={BadgeType.PrimarySoft}
                                        />
                                    </div>
                                }
                            >
                                <div className="flex flex-col gap-4">
                                    <ObjectDetail
                                        objectId={controller.objectId}
                                        objectType={controller.objectType!}
                                    />
                                </div>
                            </CollapsibleCard>
                        )}
                    </>
                ))}
            </div>
        </div>
    );
}

interface ControllerCardFooterProps {
    weight: number;
    threshold: string | null;
    ownerType: string;
    ownerAddress: string;
}

// NOTE: This is a copy of ObjectChangeEntriesCardFooter
export function ControllerCardFooter({
    weight,
    threshold,
    ownerType,
    ownerAddress,
}: ControllerCardFooterProps): JSX.Element {
    return (
        <>
            <div className="flex flex-wrap px-md--rs py-sm--rs">
                <KeyValueInfo
                    keyText="Weight"
                    value={[`${weight}`, threshold && ` of ${threshold}`]}
                    fullwidth
                />
            </div>
            <div className="flex flex-wrap justify-between px-md--rs py-sm--rs">
                <span className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                    Owner
                </span>
                <div className="flex flex-row items-center gap-xs">
                    {ownerType === 'AddressOwner' && (
                        <AddressLink
                            address={ownerAddress}
                            copyText={ownerAddress}
                            className="[&>div]:max-w-[200px] [&>div]:truncate"
                            display="block"
                        />
                    )}
                    {ownerType === 'ObjectOwner' && (
                        <ObjectLink objectId={ownerAddress} copyText={ownerAddress} />
                    )}
                    {ownerType === 'Shared' && (
                        <ObjectLink
                            objectId={ownerAddress}
                            label="Shared"
                            showAddressAlias={false}
                        />
                    )}
                </div>
            </div>
        </>
    );
}
