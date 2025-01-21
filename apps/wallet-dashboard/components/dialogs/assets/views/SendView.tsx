// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AddressInput, NftImage, useNftDetails } from '@iota/core';
import { useFormikContext } from 'formik';
import { DialogLayoutFooter, DialogLayoutBody } from '../../layout';
import { Button, ButtonHtmlType, Header, Title } from '@iota/apps-ui-kit';
import { Loader } from '@iota/apps-ui-icons';

interface SendViewProps {
    objectId: string;
    senderAddress: string;
    onClose: () => void;
    onBack: () => void;
}

export function SendView({ objectId, senderAddress, onClose, onBack }: SendViewProps) {
    const { isValid, dirty, isSubmitting, submitForm } = useFormikContext();
    const { nftName, nftImageUrl } = useNftDetails(objectId, senderAddress);

    return (
        <>
            <Header title="Send asset" onClose={onClose} titleCentered onBack={onBack} />
            <DialogLayoutBody>
                <div className="flex w-full flex-col items-center justify-center gap-xs">
                    <div className="w-[172px]">
                        <NftImage src={nftImageUrl} title={nftName || 'NFT'} isHoverable={false} />
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
