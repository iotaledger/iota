// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, Header, Title, TitleSize, TooltipPosition } from '@iota/apps-ui-kit';
import { useIsValidatorCommitteeMember, Validator } from '@iota/core';
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
    const { isCommitteeMember } = useIsValidatorCommitteeMember();

    const committeeMemberValidators = validators.filter((validator) =>
        isCommitteeMember(validator),
    );
    const nonCommitteeMemberValidators = validators.filter(
        (validator) => !isCommitteeMember(validator),
    );

    return (
        <DialogLayout>
            <Header title="Validator" onClose={handleClose} onBack={handleClose} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col gap-md">
                    <div className="flex w-full flex-col">
                        {committeeMemberValidators.map((validator) => (
                            <div key={validator}>
                                <Validator
                                    address={validator}
                                    onClick={() => onSelect(validator)}
                                    isSelected={selectedValidator === validator}
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
                            <div key={validator}>
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
