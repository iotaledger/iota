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
import {
    type DisplayFieldsResponse,
    type IotaObjectChange,
    type IotaObjectChangePublished,
} from '@iota/iota-sdk/client';
import { formatAddress, parseStructTag } from '@iota/iota-sdk/utils';
import clsx from 'clsx';
import { useState, type ReactNode } from 'react';
import {
    AddressLink,
    CollapsibleCard,
    ExpandableList,
    ExpandableListControl,
    ExpandableListItems,
    ObjectLink,
} from '~/components/ui';
import { ObjectDisplay } from './ObjectDisplay';
import { Badge, BadgeType, KeyValueInfo, TitleSize } from '@iota/apps-ui-kit';
import { FieldCollapsible } from '~/components';
import { TriangleDown } from '@iota/ui-icons';

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
    function getValueData() {
        switch (label) {
            case ItemLabel.Package:
                return {
                    text: packageId ? formatAddress(packageId) : '',
                    link: packageId ? `/object/${packageId}` : undefined,
                };
            case ItemLabel.Module:
                return {
                    text: moduleName || '',
                    link:
                        packageId && moduleName
                            ? `/object/${packageId}?module=${moduleName}`
                            : undefined,
                };
            case ItemLabel.Type:
                return {
                    text: typeName || '',
                    link: undefined,
                };
            default:
                return {
                    text: '',
                    link: undefined,
                };
        }
    }
    const { text: valueText, link: valueLink } = getValueData();

    return <KeyValueInfo keyText={label} valueText={valueText} valueLink={valueLink} fullwidth />;
}

interface ObjectDetailPanelProps {
    panelContent: ReactNode;
    headerContent?: ReactNode;
    hideBorder?: boolean;
}

function ObjectDetailPanel({ panelContent, headerContent }: ObjectDetailPanelProps): JSX.Element {
    const [open, setOpen] = useState(false);
    return (
        <FieldCollapsible
            hideBorder
            onOpenChange={(isOpen) => setOpen(isOpen)}
            hideArrow
            render={() => (
                <div className="flex w-full flex-row items-center justify-between">
                    <div className="flex flex-row gap-xxxs pl-xxs text-neutral-40 dark:text-neutral-60">
                        <span className="text-body-md">Object</span>

                        <TriangleDown
                            className={clsx(
                                'h-5 w-5',
                                open
                                    ? 'rotate-0 transition-transform ease-linear'
                                    : '-rotate-90 transition-transform ease-linear',
                            )}
                        />
                    </div>
                    <div className="flex flex-row items-center gap-xxs pr-xxs">{headerContent}</div>
                </div>
            )}
            open={open}
        >
            {panelContent}
        </FieldCollapsible>
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
                <div className="flex shrink-0 items-center gap-xxs">
                    <Badge type={BadgeType.PrimarySoft} label={name} />
                    {objectId && <ObjectLink objectId={objectId} />}
                </div>
            }
            panelContent={
                <div className="mt-2 flex flex-col gap-xs capitalize">
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
        <div className="px-md">
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
        </div>
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
        <div className="flex flex-wrap justify-between px-md--rs py-sm--rs">
            <span className="text-body-md text-neutral-40 dark:text-neutral-60">Owner</span>
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
                        title="Object Changes"
                        titleSize={TitleSize.Small}
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
