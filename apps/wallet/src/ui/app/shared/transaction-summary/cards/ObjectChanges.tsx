// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ExplorerLink, ExplorerLinkType } from '_components';
import {
    getObjectChangeLabel,
    type ObjectChangesByOwner,
    type ObjectChangeSummary,
    type IotaObjectChangeTypes,
    type IotaObjectChangeWithDisplay,
} from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';
import cx from 'clsx';
import { ExpandableList } from '../../ExpandableList';
import { ObjectChangeDisplay } from './objectSummary/ObjectChangeDisplay';
import { Collapsible } from '../../collapse';
import {
    Badge,
    BadgeType,
    Divider,
    KeyValueInfo,
    Panel,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import { useState } from 'react';
import { TriangleDown } from '@iota/ui-icons';

interface ObjectDetailProps {
    change: IotaObjectChangeWithDisplay;
    ownerKey: string;
    display?: boolean;
}

export function ObjectDetail({ change, display }: ObjectDetailProps) {
    if (change.type === 'transferred' || change.type === 'published') {
        return null;
    }

    const [open, setOpen] = useState(false);
    const [packageId, moduleName, typeName] = change.objectType?.split('<')[0]?.split('::') || [];

    return (
        <Collapsible
            hideBorder
            onOpenChange={(isOpen) => setOpen(isOpen)}
            hideArrow
            render={() => (
                <div className="flex w-full flex-row items-center justify-between">
                    <Title
                        size={TitleSize.Small}
                        title="Object"
                        trailingElement={
                            <TriangleDown
                                className={cx(
                                    'ml-xxxs h-5 w-5 text-neutral-60',
                                    open
                                        ? 'rotate-0 transition-transform ease-linear'
                                        : '-rotate-90 transition-transform ease-linear',
                                )}
                            />
                        }
                    />
                    <div className="flex flex-row items-center gap-xxs pr-md">
                        <Badge type={BadgeType.PrimarySoft} label={typeName} />
                        {change.objectId && (
                            <KeyValueInfo
                                keyText="Package"
                                value={
                                    <ExplorerLink
                                        type={ExplorerLinkType.Object}
                                        objectID={change.objectId}
                                    >
                                        {formatAddress(packageId)}
                                    </ExplorerLink>
                                }
                                fullwidth
                            />
                        )}
                    </div>
                </div>
            )}
        >
            <div className="flex flex-col gap-y-sm px-md">
                <KeyValueInfo
                    keyText="Package"
                    value={
                        <ExplorerLink
                            objectID={packageId}
                            type={ExplorerLinkType.Object}
                            moduleName={moduleName}
                        >
                            {formatAddress(packageId)}
                        </ExplorerLink>
                    }
                    fullwidth
                />

                <KeyValueInfo
                    keyText="Module"
                    value={
                        <ExplorerLink
                            objectID={packageId}
                            type={ExplorerLinkType.Object}
                            moduleName={moduleName}
                        >
                            {moduleName}
                        </ExplorerLink>
                    }
                    fullwidth
                />
                <KeyValueInfo
                    keyText="Type"
                    value={
                        <ExplorerLink
                            objectID={packageId}
                            type={ExplorerLinkType.Object}
                            moduleName={moduleName}
                        >
                            {typeName}
                        </ExplorerLink>
                    }
                    fullwidth
                />
            </div>
        </Collapsible>
    );
}

interface ObjectChangeEntryProps {
    type: IotaObjectChangeTypes;
    changes: ObjectChangesByOwner;
}

export function ObjectChangeEntry({ changes, type }: ObjectChangeEntryProps) {
    const [open, setOpen] = useState(new Set());
    return (
        <>
            {Object.entries(changes).map(([owner, changes]) => {
                const label = getObjectChangeLabel(type);
                const isOpen = open.has(owner);

                return (
                    <Panel key={`${type}-${owner}`} hasBorder>
                        <div className="flex flex-col gap-y-sm overflow-hidden rounded-xl">
                            <Collapsible
                                hideBorder
                                defaultOpen
                                onOpenChange={(isOpen) => {
                                    setOpen((set) => {
                                        if (isOpen) {
                                            set.add(owner);
                                        } else {
                                            set.delete(owner);
                                        }
                                        return new Set(set);
                                    });
                                }}
                                render={() => (
                                    <Title
                                        size={TitleSize.Small}
                                        title="Object Changes"
                                        trailingElement={
                                            <div className="ml-1 flex">
                                                <Badge type={BadgeType.PrimarySoft} label={label} />
                                            </div>
                                        }
                                    />
                                )}
                            >
                                <>
                                    {!!changes.changesWithDisplay.length && (
                                        <div className="flex flex-1 flex-col gap-2 overflow-y-auto">
                                            <ExpandableList
                                                defaultItemsToShow={5}
                                                items={
                                                    isOpen
                                                        ? changes.changesWithDisplay.map(
                                                              (change) => (
                                                                  <ObjectChangeDisplay
                                                                      change={change}
                                                                  />
                                                              ),
                                                          )
                                                        : []
                                                }
                                            />
                                        </div>
                                    )}

                                    <div className="flex w-full flex-col gap-2">
                                        <ExpandableList
                                            defaultItemsToShow={5}
                                            items={
                                                open
                                                    ? changes.changes.map((change) => (
                                                          <ObjectDetail
                                                              ownerKey={owner}
                                                              change={change}
                                                          />
                                                      ))
                                                    : []
                                            }
                                        />
                                    </div>
                                </>
                            </Collapsible>
                            <div className="flex flex-col gap-y-sm px-md pb-md">
                                <Divider />
                                <KeyValueInfo
                                    keyText="Owner"
                                    value={
                                        <ExplorerLink
                                            type={ExplorerLinkType.Address}
                                            address={owner}
                                        >
                                            {owner}
                                        </ExplorerLink>
                                    }
                                    fullwidth
                                />
                            </div>
                        </div>
                    </Panel>
                );
            })}
        </>
    );
}

interface ObjectChangesProps {
    changes?: ObjectChangeSummary | null;
}

export function ObjectChanges({ changes }: ObjectChangesProps) {
    if (!changes) return null;

    return (
        <>
            {Object.entries(changes).map(([type, changes]) => {
                return (
                    <ObjectChangeEntry
                        key={type}
                        type={type as keyof ObjectChangeSummary}
                        changes={changes}
                    />
                );
            })}
        </>
    );
}
