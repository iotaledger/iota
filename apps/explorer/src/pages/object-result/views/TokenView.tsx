// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    ButtonSegment,
    ButtonSegmentType,
    SegmentedButton,
    SegmentedButtonType,
} from '@iota/apps-ui-kit';
import { useGetDynamicFields, useGetObjectOrPastObject } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { type IotaObjectResponse } from '@iota/iota-sdk/client';
import { useState } from 'react';
import { DynamicFieldsCard, ObjectFieldsCard, TransactionBlocksForAddress } from '~/components';

enum FieldCategory {
    Default = 'fields',
    Dynamic = 'dynamicFields',
}

function useObjectFieldsCard(id: string) {
    const { data: iotaObjectResponseData, isPending, isError } = useGetObjectOrPastObject(id);

    const objectType =
        (iotaObjectResponseData?.data?.type ??
        iotaObjectResponseData?.data?.content?.dataType === 'package')
            ? iotaObjectResponseData.data.type
            : iotaObjectResponseData?.data?.content?.type;

    const [packageId, moduleName, functionName] = objectType?.split('<')[0]?.split('::') || [];

    // Get the normalized struct for the object
    const {
        data: normalizedStructData,
        isLoading: loadingNormalizedStruct,
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

interface FieldsContentProps {
    objectId: string;
}

enum FieldCategory {
    Fields = 'fields',
    DynamicFields = 'dynamicFields',
}

export function FieldsContent({ objectId }: FieldsContentProps) {
    const {
        normalizedStructData,
        iotaObjectResponseData,
        objectType,
        loading: objectFieldsCardLoading,
        isError: objectFieldsCardError,
    } = useObjectFieldsCard(objectId);

    const fieldsCount = normalizedStructData?.fields.length;
    const FIELDS_CATEGORIES = [
        {
            label: `${fieldsCount !== undefined ? `${fieldsCount} ` : ''}Fields`,
            value: FieldCategory.Fields,
        },
        {
            label: 'Dynamic Fields',
            value: FieldCategory.Dynamic,
        },
    ];

    const [activeTab, setActiveTab] = useState<string>(FieldCategory.Fields);

    const { data: dynamicFieldsData } = useGetDynamicFields(objectId);

    const renderDynamicFields = !!dynamicFieldsData?.pages?.[0].data.length;

    return (
        <div>
            <SegmentedButton type={SegmentedButtonType.Transparent}>
                {FIELDS_CATEGORIES.map(({ label, value }) => (
                    <ButtonSegment
                        key={value}
                        onClick={() => setActiveTab(value)}
                        label={label}
                        selected={activeTab === value}
                        type={ButtonSegmentType.Underlined}
                        disabled={value === FieldCategory.Dynamic && !renderDynamicFields}
                    />
                ))}
            </SegmentedButton>
            <div className="flex flex-col gap-5 p-md">
                {activeTab === FieldCategory.Fields && (
                    <ObjectFieldsCard
                        objectType={objectType || ''}
                        normalizedStructData={normalizedStructData}
                        iotaObjectResponseData={iotaObjectResponseData}
                        loading={objectFieldsCardLoading}
                        error={objectFieldsCardError}
                        id={objectId}
                    />
                )}
                {activeTab === FieldCategory.Dynamic && <DynamicFieldsCard id={objectId} />}
            </div>
        </div>
    );
}

interface TokenViewProps {
    data: IotaObjectResponse;
}

export function TokenView({ data }: TokenViewProps): JSX.Element {
    // eslint-disable-next-line @typescript-eslint/no-non-null-asserted-optional-chain
    const objectId = data.data?.objectId!;

    return (
        <div className="flex flex-col gap-y-2xl">
            <FieldsContent objectId={objectId} />
            <TransactionBlocksForAddress address={objectId} header="Transaction Blocks" />
        </div>
    );
}
