// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import BottomMenuLayout, { Content, Menu } from '_app/shared/bottom-menu-layout';
import { Button } from '_app/shared/ButtonUI';
import { InputWithActionButton } from '_app/shared/InputWithAction';
import { Text } from '_app/shared/text';
import { IconTooltip } from '_app/shared/tooltip';
import Alert from '_components/alert';
import { IconButton } from '_components/IconButton';
import Overlay from '_components/overlay';
import { DescriptionItem } from '_pages/approval-request/transaction-request/DescriptionList';
import { type FormValues } from '_pages/swap/constants';
import { Settings16 } from '@iota/icons/src';
import { useFormContext } from 'react-hook-form';

const MAX_SLIPPAGE_COPY =
    'Slippage % is the difference between the price you expect to pay or receive for a coin when you initiate a transaction and the actual price at which the transaction is executed.';

export function MaxSlippage({ onOpen }: { onOpen: () => void }) {
    const { watch } = useFormContext<FormValues>();
    const allowedMaxSlippagePercentage = watch('allowedMaxSlippagePercentage');

    return (
        <DescriptionItem
            title={
                <div className="flex items-center gap-1">
                    <Text variant="bodySmall">Max Slippage Tolerance</Text>
                    <div>
                        <IconTooltip tip={MAX_SLIPPAGE_COPY} />
                    </div>
                </div>
            }
        >
            <div className="flex items-center gap-1">
                <Text variant="bodySmall" color="hero-dark">
                    {allowedMaxSlippagePercentage}%
                </Text>

                <IconButton
                    onClick={onOpen}
                    icon={<Settings16 className="h-4 w-4 text-hero-dark" />}
                />
            </div>
        </DescriptionItem>
    );
}

export function MaxSlippageModal({ isOpen, onClose }: { onClose: () => void; isOpen: boolean }) {
    const {
        register,
        watch,
        formState: { errors },
    } = useFormContext<FormValues>();

    const errorString = errors.allowedMaxSlippagePercentage?.message;
    const allowedMaxSlippagePercentage = watch('allowedMaxSlippagePercentage');

    return (
        <Overlay showModal={isOpen} title="Max Slippage Tolerance" closeOverlay={onClose}>
            <div className="flex h-full w-full flex-col">
                <BottomMenuLayout>
                    <Content>
                        <div>
                            <div className="mb-2.5 ml-3">
                                <Text variant="caption" weight="semibold" color="steel">
                                    your max slippage tolerance
                                </Text>
                            </div>
                            <InputWithActionButton
                                {...register('allowedMaxSlippagePercentage')}
                                value={allowedMaxSlippagePercentage}
                                placeholder="0.0"
                                suffix="%"
                            />
                            {errorString ? (
                                <div className="mt-3">
                                    <Alert>{errorString}</Alert>
                                </div>
                            ) : null}
                            <div className="ml-3 mt-3">
                                <Text variant="pSubtitle" weight="normal" color="steel-dark">
                                    {MAX_SLIPPAGE_COPY}
                                </Text>
                            </div>
                        </div>
                    </Content>

                    <Menu stuckClass="sendCoin-cta" className="mx-0 w-full gap-2.5 px-0 pb-0">
                        <Button
                            type="submit"
                            variant="primary"
                            size="tall"
                            text="Save"
                            onClick={onClose}
                        />
                    </Menu>
                </BottomMenuLayout>
            </div>
        </Overlay>
    );
}
