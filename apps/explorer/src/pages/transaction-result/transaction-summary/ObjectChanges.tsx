// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ObjectChangeLabels,
    useResolveIotaNSName,
    type IotaObjectChangeTypes,
    type IotaObjectChangeWithDisplay,
    type ObjectChangesByOwner,
    type ObjectChangeSummary,
} from '@iota/core';
import { ChevronRight12 } from '@iota/icons';
import {
    type DisplayFieldsResponse,
    type IotaObjectChange,
    type IotaObjectChangePublished,
} from '@iota/iota-sdk/client';
import { parseStructTag } from '@iota/iota-sdk/utils';
import { Text } from '@iota/ui';
import * as Collapsible from '@radix-ui/react-collapsible';
import clsx from 'clsx';
import { useState, type ReactNode } from 'react';

import {
    AddressLink,
    CollapsibleCard,
    CollapsibleSection,
    ExpandableList,
    ExpandableListControl,
    ExpandableListItems,
    ObjectLink,
} from '~/components/ui';
import { ObjectDisplay } from './ObjectDisplay';
import { Badge, BadgeType } from '@iota/apps-ui-kit';

interface ItemProps {
    label: string;
    packageId?: string;
    moduleName?: string;
    typeName?: string;
}

enum ItemLabel {
    Package = 'package',
    Module = 'module',
    Type = 'type',
}

const DEFAULT_ITEMS_TO_SHOW = 5;

function Item({ label, packageId, moduleName, typeName }: ItemProps): JSX.Element | null {
    return (
        <div
            className={clsx(
                'flex justify-between gap-10',
                label === ItemLabel.Type ? 'items-start' : 'items-center',
            )}
        >
            <Text variant="pBody/medium" color="steel-dark">
                {label}
            </Text>

            {label === ItemLabel.Package && packageId && <ObjectLink objectId={packageId} />}
            {label === ItemLabel.Module && (
                <ObjectLink objectId={`${packageId}?module=${moduleName}`} label={moduleName} />
            )}
            {label === ItemLabel.Type && (
                <div className="break-all text-right">
                    <Text mono variant="pBody/medium" color="steel-darker">
                        {typeName}
                    </Text>
                </div>
            )}
        </div>
    );
}

interface ObjectDetailPanelProps {
    panelContent: ReactNode;
    headerContent?: ReactNode;
}

function ObjectDetailPanel({ panelContent, headerContent }: ObjectDetailPanelProps): JSX.Element {
    const [open, setOpen] = useState(false);
    return (
        <Collapsible.Root open={open} onOpenChange={setOpen}>
            <div className="flex flex-wrap items-center justify-between">
                <Collapsible.Trigger>
                    <div className="flex items-center gap-0.5">
                        <Text variant="pBody/medium" color="steel-dark">
                            Object
                        </Text>

                        <ChevronRight12
                            className={clsx('h-3 w-3 text-steel-dark', open && 'rotate-90')}
                        />
                    </div>
                </Collapsible.Trigger>
                {headerContent}
            </div>

            <Collapsible.Content>
                <div className="flex flex-col gap-2">{panelContent}</div>
            </Collapsible.Content>
        </Collapsible.Root>
    );
}

interface ObjectDetailProps {
    objectType: string;
    objectId: string;
    display?: DisplayFieldsResponse;
}

function ObjectDetail({ objectType, objectId, display }: ObjectDetailProps): JSX.Element | null {
    const separator = '::';
    const objectTypeSplit = objectType?.split(separator) || [];
    const typeName = objectTypeSplit.slice(2).join(separator);
    const { address, module, name } = parseStructTag(objectType);

    const objectDetailLabels = [ItemLabel.Package, ItemLabel.Module, ItemLabel.Type];

    if (display?.data) return <ObjectDisplay display={display} objectId={objectId} />;

    return (
        <ObjectDetailPanel
            headerContent={
                <div className="flex items-center">
                    <Text mono variant="body/medium" color="steel-dark">
                        {name}:
                    </Text>
                    {objectId && <ObjectLink objectId={objectId} />}
                </div>
            }
            panelContent={
                <div className="mt-2 flex flex-col gap-2 capitalize">
                    {objectDetailLabels.map((label) => (
                        <Item
                            key={label}
                            label={label}
                            packageId={address}
                            moduleName={module}
                            typeName={typeName}
                        />
                    ))}
                </div>
            }
        />
    );
}

interface ObjectChangeEntriesProps {
    type: IotaObjectChangeTypes;
    changeEntries: IotaObjectChange[];
    isDisplay?: boolean;
}

function ObjectChangeEntries({
    changeEntries,
    type,
    isDisplay,
}: ObjectChangeEntriesProps): JSX.Element {
    const title = ObjectChangeLabels[type];
    let expandableItems = [];

    if (type === 'published') {
        expandableItems = (changeEntries as IotaObjectChangePublished[]).map(
            ({ packageId, modules }) => (
                <ObjectDetailPanel
                    key={packageId}
                    panelContent={
                        <div className="mt-2 flex flex-col gap-2">
                            <Item label={ItemLabel.Package} packageId={packageId} />
                            {modules.map((moduleName, index) => (
                                <Item
                                    key={index}
                                    label={ItemLabel.Module}
                                    moduleName={moduleName}
                                    packageId={packageId}
                                />
                            ))}
                        </div>
                    }
                />
            ),
        );
    } else {
        expandableItems = (changeEntries as IotaObjectChangeWithDisplay[]).map((change) =>
            'objectId' in change && change.display ? (
                <ObjectDisplay
                    key={change.objectId}
                    objectId={change.objectId}
                    display={change.display}
                />
            ) : (
                'objectId' in change && (
                    <ObjectDetail
                        key={change.objectId}
                        objectId={change.objectId}
                        objectType={change.objectType}
                        display={change.display}
                    />
                )
            ),
        );
    }

    return (
        <CollapsibleSection>
            <ExpandableList
                items={expandableItems}
                defaultItemsToShow={DEFAULT_ITEMS_TO_SHOW}
                itemsLabel="Objects"
            >
                <div
                    className={clsx('flex gap-2 overflow-y-auto', {
                        'flex-row': isDisplay,
                        'flex-col': !isDisplay,
                    })}
                >
                    <ExpandableListItems />
                </div>

                {changeEntries.length > DEFAULT_ITEMS_TO_SHOW && (
                    <div className="pt-4">
                        <ExpandableListControl />
                    </div>
                )}
            </ExpandableList>
        </CollapsibleSection>
    );
}

interface ObjectChangeEntriesCardFooterProps {
    ownerType: string;
    ownerAddress: string;
}

function ObjectChangeEntriesCardFooter({
    ownerType,
    ownerAddress,
}: ObjectChangeEntriesCardFooterProps): JSX.Element {
    const { data: iotansDomainName } = useResolveIotaNSName(ownerAddress);

    return (
        <div className="flex flex-wrap items-center justify-between">
            <Text variant="pBody/medium" color="steel-dark">
                Owner
            </Text>

            {ownerType === 'AddressOwner' && (
                <AddressLink label={iotansDomainName || undefined} address={ownerAddress} />
            )}

            {ownerType === 'ObjectOwner' && <ObjectLink objectId={ownerAddress} />}

            {ownerType === 'Shared' && <ObjectLink objectId={ownerAddress} label="Shared" />}
        </div>
    );
}

interface ObjectChangeEntriesCardsProps {
    data: ObjectChangesByOwner;
    type: IotaObjectChangeTypes;
}

export function ObjectChangeEntriesCards({ data, type }: ObjectChangeEntriesCardsProps) {
    if (!data) return null;
    const badgeLabel = ObjectChangeLabels[type];
    return (
        <>
            {Object.entries(data).map(([ownerAddress, changes]) => {
                const renderFooter = ['AddressOwner', 'ObjectOwner', 'Shared'].includes(
                    changes.ownerType,
                );
                return (
                    <CollapsibleCard
                        collapsible
                        key={ownerAddress}
                        title="Changes"
                        footer={
                            renderFooter && (
                                <ObjectChangeEntriesCardFooter
                                    ownerType={changes.ownerType}
                                    ownerAddress={ownerAddress}
                                />
                            )
                        }
                        supportingTitleElement={
                            <Badge label={badgeLabel} type={BadgeType.PrimarySoft} />
                        }
                    >
                        <div className="flex flex-col gap-4">
                            {!!changes.changesWithDisplay.length && (
                                <ObjectChangeEntries
                                    changeEntries={changes.changesWithDisplay}
                                    type={type}
                                    isDisplay
                                />
                            )}
                            {!!changes.changes.length && (
                                <ObjectChangeEntries changeEntries={changes.changes} type={type} />
                            )}
                        </div>
                    </CollapsibleCard>
                );
            })}
        </>
    );
}

interface ObjectChangesProps {
    objectSummary: ObjectChangeSummary;
}

export function ObjectChanges({ objectSummary }: ObjectChangesProps): JSX.Element | null {
    if (!objectSummary) return null;

    return (
        <>
            {Object.entries(objectSummary).map(([type, changes]) => (
                <ObjectChangeEntriesCards
                    key={type}
                    type={type as IotaObjectChangeTypes}
                    data={changes}
                />
            ))}
        </>
    );
}
