// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { SelectorField } from '@/components/molecules/selector-field/SelectorField';
import { Fragment, useCallback, useState } from 'react';
import { ListItem } from '@/lib';
import { IotaLogoMark, PlaceholderReplace } from '@iota/ui-icons';

const meta = {
    component: SelectorField,
    tags: ['autodocs'],
} satisfies Meta<typeof SelectorField>;

export default meta;

type Story = StoryObj<typeof meta>;

type CustomOption = {
    label: string;
    Icon: () => React.ReactNode;
};

const dropdownOptions: CustomOption[] = [
    {
        label: 'IOTA',
        Icon: () => <IotaLogoMark />,
    },
    {
        label: 'Shimmer',
        Icon: () => <PlaceholderReplace />,
    },
];

function OptionTile({ option }: { option: CustomOption }) {
    const TileIcon = option.Icon;
    return (
        <div className="flex flex-row items-center gap-x-3">
            <TileIcon />
            <span>{option.label}</span>
        </div>
    );
}

export const Default: Story = {
    args: {
        label: 'Selector Field',
        supportingText: 'Info',
        caption: 'Caption',
    },
    render: (args) => {
        const [errorMessage, setErrorMessage] = useState<string | undefined>(undefined);
        const [isOpen, setIsOpen] = useState<boolean>(false);
        const [selectorContent, setSelectorContent] = useState<React.ReactNode | undefined>(
            'Select an option',
        );

        const handleOptionClick = useCallback(
            (option: CustomOption) => {
                setErrorMessage(undefined);
                setSelectorContent(<OptionTile option={option} />);
                setIsOpen(false);
                if (option.label === 'Shimmer') {
                    setErrorMessage('Shimmer is not supported');
                }
            },
            [isOpen],
        );

        return (
            <div className="h-60">
                <SelectorField
                    {...args}
                    isOpen={isOpen}
                    setIsOpen={setIsOpen}
                    errorMessage={errorMessage}
                    dropdownElements={
                        <Fragment>
                            {dropdownOptions.map((option, index) => (
                                <ListItem
                                    key={option.label}
                                    hideBottomBorder
                                    onClick={() => handleOptionClick(option)}
                                >
                                    <OptionTile option={option} />
                                </ListItem>
                            ))}
                        </Fragment>
                    }
                >
                    {selectorContent}
                </SelectorField>
            </div>
        );
    },
};
