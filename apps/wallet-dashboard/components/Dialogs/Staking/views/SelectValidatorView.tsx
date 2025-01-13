// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button, Header } from '@iota/apps-ui-kit';

import { Validator } from './Validator';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';

interface SelectValidatorViewProps {
    validators: string[];
    onSelect: (validator: string) => void;
    onNext: () => void;
    selectedValidator: string;
    handleClose: () => void;
}

export function SelectValidatorView({
    validators,
    onSelect,
    onNext,
    selectedValidator,
    handleClose,
}: SelectValidatorViewProps): JSX.Element {
    return (
        <DialogLayout>
            <Header title="Validator" onClose={handleClose} onBack={handleClose} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col gap-md">
                    <div className="flex w-full flex-col">
                        {validators.map((validator) => (
                            <Validator
                                key={validator}
                                address={validator}
                                onClick={onSelect}
                                isSelected={selectedValidator === validator}
                            />
                        ))}
                    </div>
                </div>
            </DialogLayoutBody>
            {!!selectedValidator && (
                <DialogLayoutFooter>
                    <Button
                        fullWidth
                        data-testid="select-validator-cta"
                        onClick={onNext}
                        text="Next"
                    />
                </DialogLayoutFooter>
            )}
        </DialogLayout>
    );
}
