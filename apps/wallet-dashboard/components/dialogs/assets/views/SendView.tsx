// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AddressInput, useNftDetails } from '@iota/core';
import { useFormikContext } from 'formik';
import { DialogLayoutFooter, DialogLayoutBody } from '../../layout';
import {
    Button,
    ButtonHtmlType,
    Header,
    VisualAssetCard,
    VisualAssetType,
    Title,
} from '@iota/apps-ui-kit';
import { Loader } from '@iota/apps-ui-icons';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IotaObjectData } from '@iota/iota-sdk/client';

interface SendViewProps {
    asset: IotaObjectData;
    onClose: () => void;
    onBack: () => void;
}

export function SendView({ asset, onClose, onBack }: SendViewProps) {
    const { isValid, dirty, isSubmitting, submitForm } = useFormikContext();

    const account = useCurrentAccount();

    const senderAddress = account?.address ?? '';
    const objectId = asset?.objectId || '';

    const { nftName, nftImageUrl } = useNftDetails(objectId, senderAddress);
    return (
        <>
            <Header title="Send asset" onClose={onClose} titleCentered onBack={onBack} />
            <DialogLayoutBody>
                <div className="flex w-full flex-col items-center justify-center gap-xs">
                    <div className="w-[172px]">
                        <VisualAssetCard
                            assetSrc={nftImageUrl}
                            assetTitle={nftName}
                            assetType={VisualAssetType.Image}
                            altText={nftName || 'NFT'}
                            isHoverable={false}
                        />
                    </div>
                    <div className="flex w-full flex-col gap-md">
                        <div className="flex flex-col items-center gap-xxxs">
                            <Title title={nftName} />
                        </div>
                        <AddressInput name="to" placeholder="Enter Address" />
                    </div>
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <Button
                    fullWidth
                    htmlType={ButtonHtmlType.Submit}
                    disabled={!(isValid && dirty) || isSubmitting}
                    text="Send"
                    icon={isSubmitting ? <Loader className="animate-spin" /> : undefined}
                    iconAfterText
                    onClick={submitForm}
                />
            </DialogLayoutFooter>
        </>
    );
}
