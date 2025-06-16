// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ObjectChangeLabels,
    useFormatCoin,
    useGetObject,
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
import {
    Accordion,
    AccordionHeader,
    AccordionContent,
    Badge,
    BadgeType,
    KeyValueInfo,
    TitleSize,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { TriangleDown } from '@iota/apps-ui-icons';
import { onCopySuccess } from '~/lib/utils';

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
    switch (label) {
        case ItemLabel.Package:
            return (
                <KeyValueInfo
                    keyText={label}
                    value={
                        <ObjectLink
                            objectId={packageId || ''}
                            label={formatAddress(packageId || '')}
                            copyText={packageId || ''}
                            onCopySuccess={onCopySuccess}
                        />
                    }
                />
            );
        case ItemLabel.Module:
            return (
                <KeyValueInfo
                    keyText={label}
                    value={
                        <ObjectLink
                            objectId={packageId ? `${packageId}?module=${moduleName}` : ''}
                            label={moduleName || ''}
                        />
                    }
                />
            );
        case ItemLabel.Type:
            return <KeyValueInfo keyText={label} value={typeName || ''} />;
        default:
            return <KeyValueInfo keyText={label} value="" />;
    }
}

interface ObjectDetailPanelProps {
    panelContent: ReactNode;
    headerContent?: ReactNode;
    hideBorder?: boolean;
}

function ObjectDetailPanel({ panelContent, headerContent }: ObjectDetailPanelProps): JSX.Element {
    const [open, setOpen] = useState(false);
    return (
        <Accordion hideBorder>
            <AccordionHeader hideArrow isExpanded={open} onToggle={() => setOpen(!open)}>
                <div className="flex w-full flex-row items-center justify-between px-md--rs">
                    <div className="flex flex-row gap-xxxs text-neutral-40 dark:text-neutral-60">
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
                    <div className="flex flex-row items-center gap-xxs truncate pr-xxs">
                        {headerContent}
                    </div>
                </div>
            </AccordionHeader>
            <AccordionContent isExpanded={open}>{panelContent}</AccordionContent>
        </Accordion>
    );
}
function ObjectDetailBalance({
    objectId,
    typeArg,
}: {
    objectId: string;
    typeArg: string;
}): JSX.Element {
    const { data: objectData, isPending } = useGetObject(objectId);
    const content = objectData?.data?.content;
    const balance =
        content?.dataType === 'moveObject' && content?.fields && 'balance' in content.fields
            ? (content.fields.balance as string)
            : BigInt(0);
    const [formatted, symbol] = useFormatCoin({ balance, coinType: typeArg });

    return isPending ? (
        <div className="mt-1 flex w-full justify-center">
            <LoadingIndicator text="Loading data" />
        </div>
    ) : (
        <KeyValueInfo keyText="Balance" value={formatted} supportingLabel={symbol} />
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
    const isIotaCoin = typeName?.startsWith('Coin');
    const typeArg = typeName?.match(/<([^>]+)>/)?.[1] || '';

    if (display?.data) return <ObjectDisplay display={display} objectId={objectId} />;

    return (
        <ObjectDetailPanel
            headerContent={
                <div className="flex shrink-0 items-center gap-xxs">
                    <Badge type={BadgeType.Neutral} label={name} />
                    {objectId && <ObjectLink objectId={objectId} />}
                </div>
            }
            panelContent={
                <div className="flex flex-col gap-xs px-md--rs py-sm--rs pr-16 capitalize">
                    {isIotaCoin && <ObjectDetailBalance objectId={objectId} typeArg={typeArg} />}
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
                        <div className="flex flex-col gap-xs px-md--rs py-sm--rs pr-16 capitalize">
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
        <ExpandableList
            items={expandableItems}
            defaultItemsToShow={DEFAULT_ITEMS_TO_SHOW}
            itemsLabel="Objects"
        >
            <div className="flex flex-col gap-2 overflow-y-auto">
                <ExpandableListItems />
            </div>

            {changeEntries.length > DEFAULT_ITEMS_TO_SHOW && (
                <div className="pt-4">
                    <ExpandableListControl />
                </div>
            )}
        </ExpandableList>
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
    return (
        <div className="flex flex-wrap justify-between px-md--rs py-sm--rs">
            <span className="text-body-md text-neutral-40 dark:text-neutral-60">Owner</span>
            {ownerType === 'AddressOwner' && (
                <AddressLink
                    label={undefined}
                    address={ownerAddress}
                    copyText={ownerAddress}
                    onCopySuccess={onCopySuccess}
                />
            )}
            {ownerType === 'ObjectOwner' && (
                <ObjectLink
                    objectId={ownerAddress}
                    copyText={ownerAddress}
                    onCopySuccess={onCopySuccess}
                />
            )}
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
                            <div className="ml-1 flex">
                                <Badge label={badgeLabel} type={BadgeType.PrimarySoft} />
                            </div>
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
