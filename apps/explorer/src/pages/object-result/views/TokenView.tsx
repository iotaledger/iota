// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetDynamicFields, useGetObject } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { type IotaObjectResponse } from '@iota/iota.js/client';
import { Heading } from '@iota/ui';
import { type ReactNode, useState } from 'react';

import { DynamicFieldsCard } from '~/components/Object/DynamicFieldsCard';
import { ObjectFieldsCard } from '~/components/Object/ObjectFieldsCard';
import TransactionBlocksForAddress from '~/components/TransactionBlocksForAddress';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '~/ui/Tabs';

function FieldsContainer({ children }: { children: ReactNode }) {
    return (
        <div className="mt-4 flex flex-col gap-5 rounded-xl border border-gray-45 bg-objectCard py-6 pl-6 pr-4">
            {children}
        </div>
    );
}

enum TabValue {
    Fields = 'fields',
    DynamicFields = 'dynamicFields',
}

function useObjectFieldsCard(id: string) {
    const { data: iotaObjectResponseData, isPending, isError } = useGetObject(id);

    const objectType =
        iotaObjectResponseData?.data?.type ??
        iotaObjectResponseData?.data?.content?.dataType === 'package'
            ? iotaObjectResponseData.data.type
            : iotaObjectResponseData?.data?.content?.type;

    const [packageId, moduleName, functionName] = objectType?.split('<')[0]?.split('::') || [];

    // Get the normalized struct for the object
    const {
        data: normalizedStructData,
        isPending: loadingNormalizedStruct,
        isError: errorNormalizedMoveStruct,
    } = useIotaClientQuery(
        'getNormalizedMoveStruct',
        {
            package: packageId,
            module: moduleName,
            struct: functionName,
        },
        {
            enabled: !!packageId && !!moduleName && !!functionName,
        },
    );

    return {
        loading: isPending || loadingNormalizedStruct,
        isError: isError || errorNormalizedMoveStruct,
        normalizedStructData,
        iotaObjectResponseData,
        objectType,
    };
}

export function FieldsContent({ objectId }: { objectId: string }) {
    const {
        normalizedStructData,
        iotaObjectResponseData,
        objectType,
        loading: objectFieldsCardLoading,
        isError: objectFieldsCardError,
    } = useObjectFieldsCard(objectId);

    const fieldsCount = normalizedStructData?.fields.length;

    const [activeTab, setActiveTab] = useState<string>(TabValue.Fields);

    const { data: dynamicFieldsData } = useGetDynamicFields(objectId);

    const renderDynamicFields = !!dynamicFieldsData?.pages?.[0].data.length;

    return (
        <Tabs size="lg" value={activeTab} onValueChange={setActiveTab}>
            <TabsList>
                <TabsTrigger value={TabValue.Fields}>
                    <Heading variant="heading4/semibold">{fieldsCount} Fields</Heading>
                </TabsTrigger>

                {renderDynamicFields && (
                    <TabsTrigger value={TabValue.DynamicFields}>
                        <Heading variant="heading4/semibold">Dynamic Fields</Heading>
                    </TabsTrigger>
                )}
            </TabsList>

            <TabsContent value={TabValue.Fields}>
                <FieldsContainer>
                    <ObjectFieldsCard
                        objectType={objectType || ''}
                        normalizedStructData={normalizedStructData}
                        iotaObjectResponseData={iotaObjectResponseData}
                        loading={objectFieldsCardLoading}
                        error={objectFieldsCardError}
                        id={objectId}
                    />
                </FieldsContainer>
            </TabsContent>
            {renderDynamicFields && (
                <TabsContent value={TabValue.DynamicFields}>
                    <FieldsContainer>
                        <DynamicFieldsCard id={objectId} />
                    </FieldsContainer>
                </TabsContent>
            )}
        </Tabs>
    );
}

export function TokenView({ data }: { data: IotaObjectResponse }) {
    // eslint-disable-next-line @typescript-eslint/no-non-null-asserted-optional-chain
    const objectId = data.data?.objectId!;

    return (
        <div className="flex flex-col flex-nowrap gap-14">
            <FieldsContent objectId={objectId} />

            <TransactionBlocksForAddress address={objectId} header="Transaction Blocks" />
        </div>
    );
}
