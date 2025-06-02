// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type BalanceChange,
    type ObjectChange,
    type ObjectChangeSummary,
    VirtualList,
    useRecognizedPackages,
    useTransactionSummary,
} from '@iota/core';
import type { OwnedObjectRef, IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { BalanceChanges } from './BalanceChanges';
import { ObjectChanges } from './ObjectChanges';
import { UpgradedSystemPackages } from './UpgradedSystemPackages';
import { useMemo } from 'react';

enum ItemType {
    Balance = 'balance',
    Object = 'object',
    Package = 'package',
}
interface TransactionSummaryProps {
    transaction: IotaTransactionBlockResponse;
}

interface BalanceItem {
    type: ItemType.Balance;
    owner: string;
    change: BalanceChange;
}

interface ObjectItem {
    type: ItemType.Object;
    changeType: keyof ObjectChangeSummary;
    objectId: string;
    change: ObjectChange;
}

interface PackageItem {
    type: ItemType.Package;
    pkg: OwnedObjectRef;
}

type ListItem = BalanceItem | ObjectItem | PackageItem;

const EMPTY_OBJECT_SUMMARY: ObjectChangeSummary = {
    transferred: {},
    published: {},
    mutated: {},
    deleted: {},
    wrapped: {},
    created: {},
};

export function TransactionSummary({ transaction }: TransactionSummaryProps): JSX.Element {
    const recognizedPackagesList = useRecognizedPackages();
    const summary = useTransactionSummary({
        transaction,
        recognizedPackagesList,
    });

    const transactionKindName = transaction.transaction?.data.transaction.kind;
    const { balanceChanges, objectSummary, upgradedSystemPackages } = summary || {};

    const items = useMemo(() => {
        const balanceItems: BalanceItem[] =
            balanceChanges && transactionKindName === 'ProgrammableTransaction'
                ? Object.entries(balanceChanges).flatMap(([owner, changes]) =>
                      changes.map((change) => ({
                          type: ItemType.Balance,
                          owner,
                          change,
                      })),
                  )
                : [];

        const objectItems: ObjectItem[] = objectSummary
            ? Object.entries(objectSummary).flatMap(([changeType, changes]) =>
                  Object.entries(changes).map(([objectId, change]) => ({
                      type: ItemType.Object,
                      changeType: changeType as keyof ObjectChangeSummary,
                      objectId,
                      change,
                  })),
              )
            : [];

        const packageItems: PackageItem[] = upgradedSystemPackages
            ? upgradedSystemPackages.map((pkg) => ({
                  type: ItemType.Package,
                  pkg,
              }))
            : [];

        return [...balanceItems, ...objectItems, ...packageItems];
    }, [balanceChanges, objectSummary, upgradedSystemPackages, transactionKindName]);

    const SIZE_MAP: { [key in ItemType]: number } = {
        [ItemType.Balance]: 200,
        [ItemType.Object]: 220,
        [ItemType.Package]: 300,
    };

    return (
        <div className="px-md--rs py-md md:py-sm">
            <VirtualList
                items={items}
                estimateSize={(index) => SIZE_MAP[items[index].type]}
                render={(item: ListItem, index: number) => {
                    switch (item.type) {
                        case ItemType.Balance:
                            return (
                                <div className="mb-sm" key={`balance-${item.owner}-${index}`}>
                                    <BalanceChanges changes={{ [item.owner]: [item.change] }} />
                                </div>
                            );
                        case ItemType.Object:
                            return (
                                <div className="mb-sm" key={`object-${item.objectId}-${index}`}>
                                    <ObjectChanges
                                        objectSummary={{
                                            ...EMPTY_OBJECT_SUMMARY,
                                            [item.changeType]: { [item.objectId]: item.change },
                                        }}
                                    />
                                </div>
                            );
                        case ItemType.Package:
                            return (
                                <div className="mb-sm" key={`package-${index}`}>
                                    <UpgradedSystemPackages
                                        data={Array.isArray(item.pkg) ? item.pkg : [item.pkg]}
                                    />
                                </div>
                            );
                        default:
                            return null;
                    }
                }}
                heightClassName="max-h-[1000px] h-full"
            />
        </div>
    );
}
