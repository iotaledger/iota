// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cl from 'clsx';
import { Button, Header } from '@iota/apps-ui-kit';
import { Validator } from '@iota/core';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { useIsValidatorCommitteeMember } from '@/hooks';

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
    const { isCommitteeMember } = useIsValidatorCommitteeMember();

    return (
        <DialogLayout>
            <Header title="Validator" onClose={handleClose} onBack={handleClose} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col gap-md">
                    <div className="flex w-full flex-col">
                        {validators.map((validator) => (
                            <div
                                key={validator}
                                className={cl({ 'opacity-50': !isCommitteeMember(validator) })}
                            >
                                <Validator
                                    address={validator}
                                    onClick={() => onSelect(validator)}
                                    isSelected={selectedValidator === validator}
                                />
                            </div>
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
