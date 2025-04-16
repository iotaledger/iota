// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    type IotaSystemStateSummaryCompat,
    useGetDynamicFields,
    useGetLatestIotaSystemState,
    useGetObject,
    useGetValidatorsApy,
    useGetValidatorsEvents,
} from '@iota/core';
import { useMemo } from 'react';
import { useParams } from 'react-router-dom';
import { InactiveValidators, PageLayout, ValidatorMeta, ValidatorStats } from '~/components';
import { VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib/constants';
import { getValidatorMoveEvent } from '~/lib/utils';
import { InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';
import { type InactiveValidatorMetaProps } from '~/components/validator/ValidatorMeta';
import { useGetInactiveValidators } from '~/hooks/useGetInactiveValidators';

const getAtRiskRemainingEpochs = (
    data: IotaSystemStateSummaryCompat | undefined,
    validatorId: string | undefined,
): number | null => {
    if (!data || !validatorId) return null;
    const atRisk = data.atRiskValidators.find(([address]) => address === validatorId);
    return atRisk ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRisk[1]) : null;
};

const getInactivePoolsId = (id: string, objectId: string): InactiveValidatorMetaProps | null => {
    //console.log('Object:', objectId);
    const { data: object } = useGetObject(objectId);
    //console.log('Object Data:', object);
    const dynamicFieldId = object?.data?.content?.fields?.value?.fields?.inner?.fields?.id?.id;
    //console.log('Dynamic Field ID:', dynamicFieldId);
    const { data: dynamicFields } = useGetDynamicFields(dynamicFieldId);
    //console.log('Dynamic Fields:', dynamicFields);
    const dfObjectId = dynamicFields?.pages?.[0]?.data?.[0]?.objectId;
    //console.log('DF Object ID:', dfObjectId);
    const { data: dfObject } = useGetObject(dfObjectId);
    const metadata = dfObject?.data?.content?.fields?.value.fields.metadata?.fields;
    if (metadata && metadata?.iota_address === id) {
        metadata.staking_pool_id = object?.data?.content?.fields?.name;
        return metadata;
    }
    return null;
};

function ValidatorDetails(): JSX.Element {
    const { id } = useParams();
    const { data, isPending } = useGetLatestIotaSystemState();
    const inactiveValidators = useGetDynamicFields(data?.inactivePoolsId ?? '');
    const maping = inactiveValidators.data?.pages?.flatMap((page) =>
        page.data.map((validator) => ({
            objectId: validator.objectId,
        })),
    );
    console.log(maping);
    console.log(maping?.length);
    let inactiveValidatorData = null;
    for (let index = 0; index < 3; index++) {
        const objectId = maping?.[index]?.objectId;
        inactiveValidatorData = getInactivePoolsId(id ?? '', objectId ?? '');
        if (inactiveValidatorData !== null) {
            break;
        } else {
            console.log('Next');
        }
    }
    console.log('Inactive Validator Data:', inactiveValidatorData);
    // const inactiveValidatorData = useGetInactiveValidators(id ?? '', maping ?? []);
    // maping?.forEach((item) => {
    //     const objectId = item.objectId;
    //     if (inactiveValidatorData !== null) {
    //         console.log('Inactive Validator Data:', inactiveValidatorData);
    //     } else {
    //         console.log('Next');
    //     }
    // });
    // console.log('Inactive Validator Data:', inactiveValidatorData);
    const validatorData = useMemo(() => {
        if (!data) return null;
        return (
            data.activeValidators.find(
                ({ iotaAddress, stakingPoolId }) => iotaAddress === id || stakingPoolId === id,
            ) || null
        );
    }, [id, data]);
    const atRiskRemainingEpochs = getAtRiskRemainingEpochs(data, id);

    const numberOfValidators = data?.activeValidators.length ?? null;
    const { data: rollingAverageApys, isPending: validatorsApysLoading } = useGetValidatorsApy();

    const { data: validatorEvents, isPending: validatorsEventsLoading } = useGetValidatorsEvents({
        limit: numberOfValidators,
        order: 'descending',
    });
    const validatorRewards = useMemo(() => {
        if (!validatorEvents || !id) return 0;
        const rewards = (
            getValidatorMoveEvent(validatorEvents, id) as { pool_staking_reward: string }
        )?.pool_staking_reward;

        return rewards ? Number(rewards) : null;
    }, [id, validatorEvents]);

    if (isPending || validatorsEventsLoading || validatorsApysLoading) {
        return <PageLayout content={<LoadingIndicator />} />;
    }

    if (!validatorData || !data || !validatorEvents || !id) {
        return (
            <PageLayout
                content={
                    <div className="mb-10">
                        <InfoBox
                            title="Inactive validator"
                            icon={<Warning />}
                            type={InfoBoxType.Warning}
                            style={InfoBoxStyle.Elevated}
                        />
                        {inactiveValidatorData && <InactiveValidators {...inactiveValidatorData} />}
                    </div>
                }
            />
        );
    }
    const { apy, isApyApproxZero } = rollingAverageApys?.[id] ?? { apy: null };

    const tallyingScore =
        (
            validatorEvents as {
                parsedJson?: { tallying_rule_global_score?: string; validator_address?: string };
            }[]
        )?.find(({ parsedJson }) => parsedJson?.validator_address === id)?.parsedJson
            ?.tallying_rule_global_score || null;

    return (
        <PageLayout
            content={
                <div className="flex flex-col gap-2xl">
                    <ValidatorMeta validatorData={validatorData} />
                    <ValidatorStats
                        validatorData={validatorData}
                        epoch={data.epoch}
                        epochRewards={validatorRewards}
                        apy={isApyApproxZero ? '~0' : apy}
                        tallyingScore={tallyingScore}
                    />
                    {atRiskRemainingEpochs !== null && (
                        <InfoBox
                            title={`At risk of being removed as a validator after ${atRiskRemainingEpochs} epoch${
                                atRiskRemainingEpochs > 1 ? 's' : ''
                            }`}
                            supportingText="Staked IOTA is below the minimum IOTA stake threshold to remain
                                    a validator."
                            icon={<Warning />}
                            type={InfoBoxType.Warning}
                            style={InfoBoxStyle.Elevated}
                        />
                    )}
                </div>
            }
        />
    );
}

export { ValidatorDetails };
