// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, KeyValueInfo, Panel } from '@iota/apps-ui-kit';
import { type IotaValidatorSummary } from '@iota/iota-sdk/client';
import toast from 'react-hot-toast';
import { ArrowTopRight } from '@iota/ui-icons';
import { AddressLink, ImageIcon, ImageIconSize } from '~/components/ui';

type ValidatorMetaProps = {
    validatorData: IotaValidatorSummary;
};

export function ValidatorMeta({ validatorData }: ValidatorMetaProps): JSX.Element {
    const validatorPublicKey = validatorData.protocolPubkeyBytes;
    const validatorName = validatorData.name;
    const logo = validatorData.imageUrl;
    const description = validatorData.description;
    const projectUrl = validatorData.projectUrl;

    function handleOnCopy() {
        toast.success('Copied to clipboard');
    }

    return (
        <div className="flex flex-col gap-y-md">
            <Panel>
                <div className="flex flex-col gap-lg p-md--rs md:flex-row">
                    <div className="flex flex-row gap-lg">
                        <div className="flex h-[120px] w-[120px]">
                            <ImageIcon
                                src={logo}
                                label={validatorName}
                                fallback={validatorName}
                                size={ImageIconSize.Full}
                            />
                        </div>
                        <div className="flex flex-col gap-y-sm">
                            <div>
                                <Badge type={BadgeType.Neutral} label="Validator" />
                            </div>
                            <div className="flex flex-row items-center gap-x-xs text-neutral-10 dark:text-neutral-92">
                                <span className="text-headline-md">{validatorName}</span>
                                {projectUrl && (
                                    <a href={projectUrl} target="_blank" rel="noreferrer noopener">
                                        <ArrowTopRight />
                                    </a>
                                )}
                            </div>
                        </div>
                    </div>
                    <div className="flex w-1/2 flex-col gap-y-md">
                        <span className="text-label-lg text-neutral-40 dark:text-neutral-60">
                            Description
                        </span>
                        <span className="text-body-md text-neutral-10 dark:text-neutral-92">
                            {description ?? '--'}
                        </span>
                    </div>
                </div>
            </Panel>
            <Panel>
                <div className="flex flex-col gap-md p-md--rs">
                    <KeyValueInfo
                        keyText="Pool ID"
                        value={validatorData.stakingPoolId}
                        copyText={validatorData.stakingPoolId}
                        onCopySuccess={handleOnCopy}
                    />
                    <KeyValueInfo
                        keyText="Address"
                        value={
                            <AddressLink
                                address={validatorData.iotaAddress}
                                label={validatorData.iotaAddress}
                            />
                        }
                        copyText={validatorData.iotaAddress}
                        onCopySuccess={handleOnCopy}
                    />
                    <KeyValueInfo
                        keyText="Public Key"
                        value={validatorPublicKey}
                        copyText={validatorPublicKey}
                    />
                </div>
            </Panel>
        </div>
    );
}
