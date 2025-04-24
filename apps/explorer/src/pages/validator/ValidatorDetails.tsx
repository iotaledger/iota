// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetValidatorsApy, useGetValidatorsEvents } from '@iota/core';
import { useParams } from 'react-router-dom';
import { InactiveValidators, PageLayout, ValidatorMeta, ValidatorStats } from '~/components';
import { VALIDATOR_LOW_STAKE_GRACE_PERIOD } from '~/lib/constants';
import { getValidatorMoveEvent } from '~/lib/utils';
import { InfoBox, InfoBoxStyle, InfoBoxType, LoadingIndicator } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';
import { useQuery } from '@tanstack/react-query';
import type { IotaClient, LatestIotaSystemStateSummary } from '@iota/iota-sdk/client';
import { normalizeIotaAddress, toB64 } from '@iota/iota-sdk/utils';
import { useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { z } from 'zod';

const getAtRiskRemainingEpochs = (
    data: LatestIotaSystemStateSummary | undefined,
    validatorId: string | undefined,
): number | null => {
    if (!data || !validatorId) return null;
    const atRisk = data.atRiskValidators.find(([address]) => address === validatorId);
    return atRisk ? VALIDATOR_LOW_STAKE_GRACE_PERIOD - Number(atRisk[1]) : null;
};

// Schema for validator object
export const ValidatorSchema = z.object({
    fields: z.object({
        name: z.string(),
        value: z.object({
            fields: z.object({
                inner: z.object({
                    fields: z.object({
                        id: z.object({
                            id: z.string(),
                        }),
                    }),
                }),
            }),
        }),
    }),
});

// Schema for dynamic field object
export const DynamicFieldObjectSchema = z.object({
    fields: z.object({
        value: z.object({
            fields: z.object({
                metadata: z.object({
                    fields: z.object({
                        image_url: z.string(),
                        name: z.string(),
                        description: z.string(),
                        project_url: z.string(),
                        protocol_pubkey_bytes: z.array(z.number()),
                        iota_address: z.string(),
                    }),
                }),
            }),
        }),
    }),
});

export type InactiveValidatorData = {
    logo: string;
    validatorName: string;
    description: string;
    projectUrl: string;
    validatorPublicKey: string;
    validatorAddress: string;
    validatorStakingPoolId: string;
};

// Function to get inactive validator data
// It fetches the validator object and its dynamic fields to extract metadata
const getInactiveValidatorsData = async (
    client: IotaClient,
    objectId: string,
): Promise<InactiveValidatorData | null> => {
    const validatorObject = await client.getObject({
        id: normalizeIotaAddress(objectId),
        options: {
            showContent: true,
        },
    });

    const validator = ValidatorSchema.safeParse(validatorObject.data?.content);
    const validatorFieldId = validator.data?.fields.value.fields.inner.fields.id.id;
    if (!validatorFieldId) {
        return null;
    }
    const dynamicFields = await client.getDynamicFields({
        parentId: normalizeIotaAddress(validatorFieldId),
        cursor: null,
        limit: 10,
    });
    const dfObjectId = dynamicFields.data?.[0]?.objectId;
    const dfObject = await client.getObject({
        id: normalizeIotaAddress(dfObjectId),
        options: {
            showContent: true,
        },
    });
    const metadata = DynamicFieldObjectSchema.safeParse(dfObject.data?.content);
    if (!metadata.data || !validator.data) {
        return null;
    }
    return {
        logo: metadata.data.fields.value.fields.metadata.fields.image_url,
        description: metadata.data.fields.value.fields.metadata.fields.description,
        validatorName: metadata.data.fields.value.fields.metadata.fields.name,
        projectUrl: metadata.data.fields.value.fields.metadata.fields.project_url,
        validatorAddress: metadata.data.fields.value.fields.metadata.fields.iota_address,
        validatorPublicKey: toB64(
            Uint8Array.from(
                metadata.data.fields.value.fields.metadata.fields.protocol_pubkey_bytes,
            ),
        ),
        validatorStakingPoolId: validator.data.fields.name,
    };
};

function ValidatorDetails(): JSX.Element {
    const { id } = useParams();
    const { data: systemStateData, isLoading: isLoadingSystemState } = useIotaClientQuery(
        'getLatestIotaSystemState',
    );

    const iotaClient = useIotaClient();

    const { data: inactiveValidatorData, isLoading: isInactiveValidatorLoading } = useQuery({
        queryKey: [systemStateData?.inactivePoolsId, id],
        async queryFn() {
            if (!systemStateData?.inactivePoolsId || !id) {
                throw Error('Missing params');
            }
            const inactiveValidators = await iotaClient.getDynamicFields({
                parentId: normalizeIotaAddress(systemStateData?.inactivePoolsId),
            });

            const pendingInactiveValidatorsData = await Promise.all(
                inactiveValidators.data.map(
                    async (validator) =>
                        await getInactiveValidatorsData(iotaClient, validator.objectId),
                ),
            );

            return pendingInactiveValidatorsData;
        },
        enabled: !!systemStateData?.inactivePoolsId && !!id,
        select(validators) {
            return validators.find((validator) => validator?.validatorAddress === id);
        },
    });

    const numberOfValidators = systemStateData?.activeValidators.length ?? null;
    const { data: rollingAverageApys, isLoading: isValidatorsApysLoading } = useGetValidatorsApy();
    const { data: validatorEvents, isLoading: isValidatorsEventsLoading } = useGetValidatorsEvents({
        limit: numberOfValidators,
        order: 'descending',
    });
    const epochId = systemStateData?.epoch;
    const validatorRewards = (() => {
        if (!validatorEvents || !id || !epochId) return 0;
        const rewards = (
            getValidatorMoveEvent(validatorEvents, id, epochId) as { pool_staking_reward: string }
        )?.pool_staking_reward;

        return rewards ? Number(rewards) : null;
    })();

    const validatorData = systemStateData?.activeValidators.find(
        ({ iotaAddress, stakingPoolId }) => iotaAddress === id || stakingPoolId === id,
    );

    const atRiskRemainingEpochs = getAtRiskRemainingEpochs(systemStateData, id);

    if (
        isLoadingSystemState ||
        isValidatorsEventsLoading ||
        isValidatorsApysLoading ||
        isInactiveValidatorLoading
    ) {
        return <PageLayout content={<LoadingIndicator />} />;
    }

    if (inactiveValidatorData) {
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
                        {inactiveValidatorData && (
                            <InactiveValidators validatorData={inactiveValidatorData} />
                        )}
                    </div>
                }
            />
        );
    }

    if (!validatorData || !systemStateData || !validatorEvents || !id) {
        return (
            <PageLayout
                content={
                    <div className="mb-10">
                        <InfoBox
                            title="Failed to load validator data"
                            supportingText={`No validator data found for ${id}`}
                            icon={<Warning />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
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
                        epoch={systemStateData.epoch}
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
