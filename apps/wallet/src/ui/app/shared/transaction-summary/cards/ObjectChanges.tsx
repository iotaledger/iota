// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ExplorerLink, ExplorerLinkType } from '_components';
import { Text } from '_src/ui/app/shared/text';
import { Disclosure } from '@headlessui/react';
import {
    getObjectChangeLabel,
    type ObjectChangesByOwner,
    type ObjectChangeSummary,
    type IotaObjectChangeTypes,
    type IotaObjectChangeWithDisplay,
} from '@iota/core';
import { ChevronDown12, ChevronRight12 } from '@iota/icons';
import { formatAddress } from '@iota/iota.js/utils';
import cx from 'clsx';

import { ExpandableList } from '../../ExpandableList';
import { Card } from '../Card';
import { OwnerFooter } from '../OwnerFooter';
import { ObjectChangeDisplay } from './objectSummary/ObjectChangeDisplay';

interface ChevronDownProps {
    expanded: boolean;
}

function ChevronDown({ expanded }: ChevronDownProps) {
    return expanded ? (
        <ChevronDown12 className="text-gray-45" />
    ) : (
        <ChevronRight12 className="text-gray-45" />
    );
}

interface ObjectDetailProps {
    change: IotaObjectChangeWithDisplay;
    ownerKey: string;
    display?: boolean;
}

export function ObjectDetail({ change, display }: ObjectDetailProps) {
    if (change.type === 'transferred' || change.type === 'published') {
        return null;
    }

    const [packageId, moduleName, typeName] = change.objectType?.split('<')[0]?.split('::') || [];

    return (
        <Disclosure>
            {({ open }) => (
                <div className="flex flex-col gap-1">
                    <div className="grid cursor-pointer grid-cols-2 overflow-auto">
                        <Disclosure.Button className="ouline-none text-steel-dark hover:text-steel-darker flex cursor-pointer select-none items-center gap-1 border-none bg-transparent p-0">
                            <Text variant="pBody" weight="medium">
                                Object
                            </Text>
                            {open ? (
                                <ChevronDown12 className="text-gray-45" />
                            ) : (
                                <ChevronRight12 className="text-gray-45" />
                            )}
                        </Disclosure.Button>
                        {change.objectId && (
                            <div className="justify-self-end">
                                <ExplorerLink
                                    type={ExplorerLinkType.Object}
                                    objectID={change.objectId}
                                    className="text-hero-dark no-underline"
                                >
                                    <Text variant="body" weight="medium" truncate mono>
                                        {formatAddress(change.objectId)}
                                    </Text>
                                </ExplorerLink>
                            </div>
                        )}
                    </div>
                    <Disclosure.Panel>
                        <div className="flex flex-col gap-1">
                            <div className="relative grid grid-cols-2 overflow-auto">
                                <Text variant="pBody" weight="medium" color="steel-dark">
                                    Package
                                </Text>
                                <div className="flex justify-end">
                                    <ExplorerLink
                                        type={ExplorerLinkType.Object}
                                        objectID={packageId}
                                        className="text-hero-dark justify-self-end overflow-auto text-captionSmall no-underline"
                                    >
                                        <Text variant="pBody" weight="medium" truncate mono>
                                            {packageId}
                                        </Text>
                                    </ExplorerLink>
                                </div>
                            </div>
                            <div className="grid grid-cols-2 overflow-auto">
                                <Text variant="pBody" weight="medium" color="steel-dark">
                                    Module
                                </Text>
                                <div className="flex justify-end">
                                    <ExplorerLink
                                        type={ExplorerLinkType.Object}
                                        objectID={packageId}
                                        moduleName={moduleName}
                                        className="text-hero-dark justify-self-end overflow-auto no-underline"
                                    >
                                        <Text variant="pBody" weight="medium" truncate mono>
                                            {moduleName}
                                        </Text>
                                    </ExplorerLink>
                                </div>
                            </div>
                            <div className="grid grid-cols-2 overflow-auto">
                                <Text variant="pBody" weight="medium" color="steel-dark">
                                    Type
                                </Text>
                                <div className="flex justify-end">
                                    <ExplorerLink
                                        type={ExplorerLinkType.Object}
                                        objectID={packageId}
                                        moduleName={moduleName}
                                        className="text-hero-dark justify-self-end overflow-auto no-underline"
                                    >
                                        <Text variant="pBody" weight="medium" truncate mono>
                                            {typeName}
                                        </Text>
                                    </ExplorerLink>
                                </div>
                            </div>
                        </div>
                    </Disclosure.Panel>
                </div>
            )}
        </Disclosure>
    );
}

interface ObjectChangeEntryProps {
    type: IotaObjectChangeTypes;
    changes: ObjectChangesByOwner;
}

export function ObjectChangeEntry({ changes, type }: ObjectChangeEntryProps) {
    return (
        <>
            {Object.entries(changes).map(([owner, changes]) => {
                return (
                    <Card
                        footer={<OwnerFooter owner={owner} ownerType={changes.ownerType} />}
                        key={`${type}-${owner}`}
                        heading="Changes"
                    >
                        <Disclosure defaultOpen>
                            {({ open }) => (
                                <div className={cx({ 'gap-4': open }, 'flex flex-col pb-3')}>
                                    <Disclosure.Button
                                        as="div"
                                        className="flex w-full cursor-pointer flex-col gap-2"
                                    >
                                        <div className="flex w-full items-center gap-2">
                                            <Text
                                                variant="body"
                                                weight="semibold"
                                                color={
                                                    type === 'created'
                                                        ? 'success-dark'
                                                        : 'steel-darker'
                                                }
                                            >
                                                {getObjectChangeLabel(type)}
                                            </Text>
                                            <div className="bg-gray-40 h-px w-full" />
                                            <ChevronDown expanded={open} />
                                        </div>
                                    </Disclosure.Button>
                                    <Disclosure.Panel as="div" className="flex flex-col gap-4">
                                        <>
                                            {!!changes.changesWithDisplay.length && (
                                                <div className="flex gap-2 overflow-y-auto">
                                                    <ExpandableList
                                                        defaultItemsToShow={5}
                                                        items={
                                                            open
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
                                    </Disclosure.Panel>
                                </div>
                            )}
                        </Disclosure>
                    </Card>
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
