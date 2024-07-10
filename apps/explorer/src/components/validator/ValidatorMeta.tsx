// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// import { ArrowUpRight12 } from '@iota/icons';
import { type IotaValidatorSummary } from '@iota/iota.js/client';
import { Heading, Text } from '@iota/ui';

import {
    AddressLink,
    CopyToClipboard,
    DescriptionItem,
    DescriptionList,
    ImageIcon,
} from '~/components/ui';

type ValidatorMetaProps = {
    validatorData: IotaValidatorSummary;
};

export function ValidatorMeta({ validatorData }: ValidatorMetaProps): JSX.Element {
    const validatorPublicKey = validatorData.protocolPubkeyBytes;
    const validatorName = validatorData.name;
    const logo = validatorData.imageUrl;
    const description = validatorData.description;
    const projectUrl = validatorData.projectUrl;

    return (
        <>
            <div className="flex basis-full gap-5 border-r border-transparent border-r-gray-45 md:mr-7.5 md:basis-1/3">
                <ImageIcon src={logo} label={validatorName} fallback={validatorName} size="xl" />
                <div className="mt-1.5 flex flex-col">
                    <Heading as="h1" variant="heading2/bold" color="gray-90">
                        {validatorName}
                    </Heading>
                    {projectUrl && (
                        <a
                            href={projectUrl}
                            target="_blank"
                            rel="noreferrer noopener"
                            className="mt-2.5 inline-flex items-center gap-1.5 text-body font-medium text-iota-dark no-underline"
                        >
                            {projectUrl.replace(/\/$/, '')}
                            {/* <ArrowUpRight12 className="text-steel" /> */}
                        </a>
                    )}
                </div>
            </div>
            <div className="min-w-0 basis-full break-words md:basis-2/3">
                <DescriptionList>
                    <DescriptionItem title="Description" align="start">
                        <Text variant="pBody/medium" color="gray-90">
                            {description || '--'}
                        </Text>
                    </DescriptionItem>
                    <DescriptionItem title="Location" align="start">
                        <Text variant="pBody/medium" color="gray-90">
                            --
                        </Text>
                    </DescriptionItem>
                    <DescriptionItem title="Pool ID" align="start">
                        <div className="flex items-start gap-1 break-all">
                            <Text variant="pBody/medium" color="steel-darker">
                                {validatorData.stakingPoolId}
                            </Text>
                            <CopyToClipboard
                                size="md"
                                color="steel"
                                copyText={validatorData.stakingPoolId}
                            />
                        </div>
                    </DescriptionItem>
                    <DescriptionItem title="Address" align="start">
                        <div className="flex items-start gap-1">
                            <AddressLink address={validatorData.iotaAddress} noTruncate />
                            <CopyToClipboard
                                size="md"
                                color="steel"
                                copyText={validatorData.iotaAddress}
                            />
                        </div>
                    </DescriptionItem>
                    <DescriptionItem title="Public Key" align="start">
                        <Text variant="pBody/medium" color="steel-darker">
                            {validatorPublicKey}
                        </Text>
                    </DescriptionItem>
                </DescriptionList>
            </div>
        </>
    );
}
