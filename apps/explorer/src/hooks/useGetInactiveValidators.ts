import { useState, useMemo } from 'react';
import { useGetObject, useGetDynamicFields } from '@iota/core';
import { type InactiveValidatorMetaProps } from '~/components';

export function useGetInactiveValidators(id: string, maping: { objectId: string }[]) {
    const [metadata, setMetadata] = useState<InactiveValidatorMetaProps | null>(null);

    const objectDataArray = useMemo(
        () => maping.map(({ objectId }) => useGetObject(objectId)),
        [maping],
    );

    const dynamicFieldsArray = useMemo(
        () =>
            objectDataArray.map(({ data: object }) => {
                const dynamicFieldId =
                    object?.data?.content?.fields?.value?.fields?.inner?.fields?.id?.id;
                return dynamicFieldId ? useGetDynamicFields(dynamicFieldId) : null;
            }),
        [objectDataArray],
    );

    useMemo(() => {
        if (!maping || maping.length === 0 || metadata) return;

        const metadataCandidate = maping.find((_, index) => {
            const objectData = objectDataArray[index]?.data;
            const dynamicFields = dynamicFieldsArray[index]?.data;

            if (!objectData || !dynamicFields) return false;

            const dfObjectId = dynamicFields?.pages?.[0]?.data?.[0]?.objectId;
            if (!dfObjectId) return false;

            const { data: dfObject } = useGetObject(dfObjectId);
            const candidate = dfObject?.data?.content?.fields?.value.fields.metadata?.fields;

            if (candidate?.iota_address === id) {
                candidate.staking_pool_id = objectData?.data?.content?.fields?.name;
                setMetadata(candidate);
                return true;
            }

            return false;
        });

        if (!metadataCandidate) {
            console.log('No matching metadata found.');
        }
    }, [id, maping, objectDataArray, dynamicFieldsArray, metadata]);

    return metadata;
}
