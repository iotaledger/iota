// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, Header, Search, Title, TitleSize, TooltipPosition } from '@iota/apps-ui-kit';
import { useIsValidatorCommitteeMember, Validator } from '@iota/core';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { useState } from 'react';

interface Validator {
    iotaAddress: string;
    name: string;
}
interface SelectValidatorViewProps {
    validators: Validator[];
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
    const [searchValidator, setSearchValidator] = useState('');

    const { isCommitteeMember } = useIsValidatorCommitteeMember();

    const filteredValidators = validators.filter((validator) => {
        const valueToLowerCase = searchValidator.toLowerCase();
        return (
            validator.name.toLowerCase().includes(valueToLowerCase) ||
            validator.iotaAddress.toLowerCase().includes(valueToLowerCase)
        );
    });

    const committeeMemberValidators = filteredValidators.filter((validator) =>
        isCommitteeMember(validator.iotaAddress),
    );
    const nonCommitteeMemberValidators = filteredValidators.filter(
        (validator) => !isCommitteeMember(validator.iotaAddress),
    );

    return (
        <DialogLayout>
            <Header title="Validator" onClose={handleClose} onBack={handleClose} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col gap-md">
                    <Search
                        searchValue={searchValidator}
                        onSearchValueChange={setSearchValidator}
                        placeholder="Search validators"
                        isLoading={false}
                    />
                    <div className="flex w-full flex-col">
                        {committeeMemberValidators.map((validator) => (
                            <div key={validator.iotaAddress}>
                                <Validator
                                    address={validator.iotaAddress}
                                    onClick={() => onSelect(validator.iotaAddress)}
                                    isSelected={selectedValidator === validator.iotaAddress}
                                />
                            </div>
                        ))}
                    </div>
                    {nonCommitteeMemberValidators.length > 0 && (
                        <Title
                            size={TitleSize.Small}
                            title="Currently not earning rewards"
                            tooltipText="These validators are not part of the committee."
                            tooltipPosition={TooltipPosition.Left}
                        />
                    )}
                    <div className="flex w-full flex-col">
                        {nonCommitteeMemberValidators.map((validator) => (
                            <div key={validator.iotaAddress}>
                                <Validator
                                    address={validator.iotaAddress}
                                    onClick={() => onSelect(validator.iotaAddress)}
                                    isSelected={selectedValidator === validator.iotaAddress}
                                />
                            </div>
                        ))}
                    </div>
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <Button
                    fullWidth
                    data-testid="select-validator-cta"
                    onClick={onNext}
                    text="Next"
                    disabled={!selectedValidator}
                />
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
