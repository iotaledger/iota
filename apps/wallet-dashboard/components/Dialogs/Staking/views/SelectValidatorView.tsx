// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button, Header } from '@iota/apps-ui-kit';

import { Validator } from './Validator';
import { Layout, LayoutBody, LayoutFooter } from './Layout';

interface SelectValidatorViewProps {
    validators: string[];
    onSelect: (validator: string) => void;
    onNext: () => void;
    selectedValidator: string;
    handleClose: () => void;
}

function SelectValidatorView({
    validators,
    onSelect,
    onNext,
    selectedValidator,
    handleClose,
}: SelectValidatorViewProps): JSX.Element {
    return (
        <Layout>
            <Header title="Validator" onClose={handleClose} onBack={handleClose} titleCentered />
            <LayoutBody>
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
            </LayoutBody>
            {!!selectedValidator && (
                <LayoutFooter>
                    <Button
                        fullWidth
                        data-testid="select-validator-cta"
                        onClick={onNext}
                        text="Next"
                    />
                </LayoutFooter>
            )}
        </Layout>
    );
}

export default SelectValidatorView;
