// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    AddressInput,
    CoinFormat,
    NftImage,
    useAssetGasBudgetEstimation,
    useFormatCoin,
    useNftDetails,
} from '@iota/core';
import { useFormikContext } from 'formik';
import { DialogLayoutFooter, DialogLayoutBody } from '../../layout';
import { Button, ButtonHtmlType, Divider, Header, KeyValueInfo, Title } from '@iota/apps-ui-kit';
import { Loader } from '@iota/apps-ui-icons';

interface SendViewProps {
    objectId: string;
    senderAddress: string;
    objectType: string;
    onClose: () => void;
    onBack: () => void;
}

export function SendView({ objectId, senderAddress, objectType, onClose, onBack }: SendViewProps) {
    const { isValid, dirty, isSubmitting, submitForm, values } = useFormikContext();
    const { data: gasBudgetEst } = useAssetGasBudgetEstimation({
        objectId,
        activeAddress: senderAddress,
        to: (values as { to: string }).to,
        objectType,
    });
    const [gasFormatted, gasSymbol] = useFormatCoin({
        balance: gasBudgetEst,
        format: CoinFormat.FULL,
    });
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
                        <Divider />
                        <KeyValueInfo
                            keyText={'Est. Gas Fees'}
                            value={gasFormatted}
                            supportingLabel={gasSymbol}
                            fullwidth
                        />
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
