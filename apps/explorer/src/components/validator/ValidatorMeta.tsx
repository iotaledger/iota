// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { Badge, BadgeType, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import { type IotaValidatorSummary } from '@iota/iota-sdk/client';
import { ArrowTopRight, IotaLogoMark } from '@iota/apps-ui-icons';
import { AddressLink } from '~/components/ui';
import { ImageIcon, ImageIconSize, useIsValidatorCommitteeMember } from '@iota/core';
import type { InactiveValidatorData } from '@iota/core/src/types';
import { onCopySuccess } from '~/lib/utils';

type ValidatorMetaProps = {
    validatorData: IotaValidatorSummary;
    atRiskRemainingEpochs?: number | null;
};

export function InactiveValidators({
    validatorData: {
        imageUrl,
        name,
        description,
        projectUrl,
        validatorPublicKey,
        validatorAddress,
        validatorStakingPoolId,
    },
}: {
    validatorData: InactiveValidatorData;
}): JSX.Element {
    return (
        <div className="flex flex-col gap-y-md">
            <Panel>
                <div className="flex flex-col gap-lg p-md--rs md:flex-row">
                    <div className="flex flex-row gap-lg">
                        <div className="flex h-[120px] w-[120px]">
                            <ImageIcon
                                src={imageUrl}
                                label={name}
                                fallback={name}
                                size={ImageIconSize.Full}
                            />
                        </div>
                        <div className="flex flex-col gap-y-sm">
                            <div>
                                <Badge type={BadgeType.Neutral} label="Validator" />
                            </div>
                            <div className="flex flex-row items-center gap-x-xs text-iota-neutral-10 dark:text-iota-neutral-92">
                                <span className="text-headline-md">{name}</span>
                                {projectUrl && (
                                    <a href={projectUrl} target="_blank" rel="noreferrer noopener">
                                        <ArrowTopRight />
                                    </a>
                                )}
                            </div>
                        </div>
                    </div>
                    <div className="flex w-full flex-col gap-y-md md:w-1/2">
                        <span className="text-label-lg text-iota-neutral-40 dark:text-iota-neutral-60">
                            Description
                        </span>
                        <span className="text-body-md text-iota-neutral-10 dark:text-iota-neutral-92">
                            {description ?? '--'}
                        </span>
                    </div>
                </div>
            </Panel>
            <Panel>
                <div className="flex flex-col gap-md p-md--rs">
                    <KeyValueInfo
                        keyText="Address"
                        value={
                            <AddressLink address={validatorAddress} copyText={validatorAddress} />
                        }
                    />
                    <KeyValueInfo
                        keyText="Pool ID"
                        value={validatorStakingPoolId}
                        copyText={validatorStakingPoolId}
                        onCopySuccess={onCopySuccess}
                    />
                    <KeyValueInfo
                        keyText="Public Key"
                        value={validatorPublicKey}
                        copyText={validatorPublicKey}
                        onCopySuccess={onCopySuccess}
                    />
                </div>
            </Panel>
        </div>
    );
}

export function ValidatorMeta({
    validatorData,
    atRiskRemainingEpochs,
}: ValidatorMetaProps): JSX.Element {
    const validatorPublicKey = validatorData.protocolPubkeyBytes;
    const validatorName = validatorData.name;
    const logo = validatorData.imageUrl;
    const description = validatorData.description;
    const projectUrl = validatorData.projectUrl;
    const { isCommitteeMember } = useIsValidatorCommitteeMember();
    const isValidatorCommitteeMember = isCommitteeMember(validatorData.iotaAddress);

    return (
        <div className="flex flex-col gap-md md:flex-row">
            <div className="flex w-full md:w-2/5">
                <Panel>
                    <div className="flex flex-col gap-lg p-md--rs">
                        <div className="flex flex-row gap-lg">
                            <div className="h-[80px] w-[80px] shrink-0">
                                <ImageIcon
                                    src={logo}
                                    label={validatorName}
                                    fallback={validatorName}
                                    size={ImageIconSize.Full}
                                />
                            </div>
                            <div className="flex min-w-0 flex-col gap-sm">
                                <div className="flex flex-row items-center gap-x-sm gap-y-xs">
                                    <span className="text-headline-md text-iota-neutral-10 dark:text-iota-neutral-92">
                                        {validatorName}
                                    </span>
                                    {projectUrl && (
                                        <a
                                            href={projectUrl}
                                            target="_blank"
                                            rel="noreferrer noopener"
                                            className="text-iota-neutral-40 hover:text-iota-neutral-10 dark:text-iota-neutral-60 dark:hover:text-iota-neutral-92"
                                        >
                                            <ArrowTopRight />
                                        </a>
                                    )}
                                </div>
                                <div className="flex flex-wrap items-center gap-xs">
                                    <Badge type={BadgeType.Neutral} label="Validator" />
                                    {isValidatorCommitteeMember ? (
                                        <Badge type={BadgeType.Success} label="Committee" />
                                    ) : (
                                        <Badge type={BadgeType.PrimarySoft} label="Active" />
                                    )}
                                    {atRiskRemainingEpochs != null && (
                                        <Badge type={BadgeType.Error} label="At Risk" />
                                    )}
                                </div>
                            </div>
                        </div>
                        {description && (
                            <p className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                                {description}
                            </p>
                        )}
                    </div>
                </Panel>
            </div>

            <Panel>
                <div className="flex flex-col gap-md p-md--rs">
                    <KeyValueInfo
                        keyText="Address"
                        value={
                            <div className="flex flex-col gap-xxs">
                                <div className="flex items-center gap-xs text-iota-neutral-40 dark:text-iota-neutral-60">
                                    <IotaLogoMark className="h-3.5 w-3.5 shrink-0" />
                                    <span className="text-body-sm">{validatorName}</span>
                                </div>
                                <AddressLink
                                    address={validatorData.iotaAddress}
                                    copyText={validatorData.iotaAddress}
                                    noTruncate
                                    showAddressAlias={false}
                                />
                            </div>
                        }
                    />
                    <KeyValueInfo
                        keyText="Pool ID"
                        value={validatorData.stakingPoolId}
                        copyText={validatorData.stakingPoolId}
                        onCopySuccess={onCopySuccess}
                    />
                    <KeyValueInfo
                        keyText="Public Key"
                        value={validatorPublicKey}
                        copyText={validatorPublicKey}
                        onCopySuccess={onCopySuccess}
                    />
                </div>
            </Panel>
        </div>
    );
}
