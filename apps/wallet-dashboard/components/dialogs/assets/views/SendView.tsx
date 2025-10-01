// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    AddressInput,
    NFTMediaDisplayCard,
    RECEIVING_ADDRESS_FIELD_IDS,
    SendNftFormValues,
    useAssetGasBudgetEstimation,
    useFormatCoin,
    useNftDetails,
    useGetIotaNameRecord,
} from '@iota/core';
import { CoinFormat } from '@iota/iota-sdk/utils';
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
    const { isValid, dirty, isSubmitting, submitForm, values } =
        useFormikContext<SendNftFormValues>();
    const { data: nameRecord } = useGetIotaNameRecord(values.to);

    const { data: gasBudgetEst } = useAssetGasBudgetEstimation({
        objectId,
        activeAddress: senderAddress,
        to: nameRecord?.targetAddress ?? values.to,
        objectType,
    });
    const [gasFormatted, gasSymbol] = useFormatCoin({
        balance: gasBudgetEst,
        format: CoinFormat.Full,
    });
    const { nftName, nftImageUrl } = useNftDetails(objectId, senderAddress);

    return (
        <>
            <Header title="Send asset" onClose={onClose} titleCentered onBack={onBack} />
            <DialogLayoutBody>
                <div className="flex w-full flex-col items-center justify-center gap-xs">
                    <div className="w-[172px]">
                        <NFTMediaDisplayCard
                            src={nftImageUrl}
                            title={nftName || 'NFT'}
                            isHoverable={false}
                        />
                    </div>
                    <div className="flex w-full flex-col gap-md">
                        <div className="flex flex-col items-center gap-xxxs break-words [&_div]:max-w-full [&_h4]:max-w-full [&_h4]:break-words">
                            <Title title={nftName} />
                        </div>
                        <AddressInput
                            {...RECEIVING_ADDRESS_FIELD_IDS}
                            placeholder="Enter Address"
                        />
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
