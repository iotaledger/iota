// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetDynamicFields, useOnScreen } from '@iota/core';
import { type DynamicFieldInfo } from '@iota/iota-sdk/client';
import { LoadingIndicator } from '@iota/ui';
import { useRef, useEffect, useState, useMemo } from 'react';
import { UnderlyingObjectCard } from './UnderlyingObjectCard';
import { FieldCollapsible } from '~/components';
import { ObjectLink } from '~/components/ui';
import { Panel } from '@iota/apps-ui-kit';

interface DynamicFieldRowProps {
    id: string;
    result: DynamicFieldInfo;
    defaultOpen: boolean;
}

function DynamicFieldRow({ id, result, defaultOpen }: DynamicFieldRowProps): JSX.Element {
    const [open, onOpenChange] = useState(defaultOpen);

    return (
        <FieldCollapsible
            open={open}
            onOpenChange={onOpenChange}
            render={({ isOpen }) => (
                <div className="flex items-center gap-xs truncate break-words pl-md--rs text-body-md text-neutral-40">
                    <div className="block w-full truncate break-words">
                        {typeof result.name?.value === 'object' ? (
                            <>Struct {result.name.type}</>
                        ) : result.name?.value ? (
                            String(result.name.value)
                        ) : null}
                    </div>
                    <ObjectLink objectId={result.objectId} />
                </div>
            )}
        >
            <div className="p-md--rs">
                <UnderlyingObjectCard
                    parentId={id}
                    name={result.name}
                    dynamicFieldType={result.type}
                />
            </div>
        </FieldCollapsible>
    );
}

export function DynamicFieldsCard({ id }: { id: string }) {
    const { data, isInitialLoading, isFetchingNextPage, hasNextPage, fetchNextPage } =
        useGetDynamicFields(id);

    const observerElem = useRef<HTMLDivElement | null>(null);
    const { isIntersecting } = useOnScreen(observerElem);
    const isSpinnerVisible = isFetchingNextPage && hasNextPage;
    const flattenedData = useMemo(() => data?.pages.flatMap((page) => page.data), [data]);

    useEffect(() => {
        if (isIntersecting && hasNextPage && !isFetchingNextPage) {
            fetchNextPage();
        }
    }, [isIntersecting, fetchNextPage, hasNextPage, isFetchingNextPage]);

    if (isInitialLoading) {
        return (
            <div className="mt-1 flex w-full justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    return (
        <div className="flex flex-col gap-md">
            <Panel hasBorder>
                <div className="flex flex-col gap-md p-md--rs">
                    {flattenedData?.map((result, index) => (
                        <DynamicFieldRow
                            key={result.objectId}
                            defaultOpen={index === 0}
                            id={id}
                            result={result}
                        />
                    ))}

                    <div ref={observerElem}>
                        {isSpinnerVisible ? (
                            <div className="mt-1 flex w-full justify-center">
                                <LoadingIndicator text="Loading data" />
                            </div>
                        ) : null}
                    </div>
                </div>
            </Panel>
        </div>
    );
}
