// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Input, InputType } from '@/lib/components/molecules/input';
import { PlaceholderReplace } from '@iota/apps-ui-icons';
import type { ComponentProps } from 'react';
import { useCallback, useEffect, useState } from 'react';
import { ButtonPill } from '@/lib/components/atoms/button';

type CustomStoryProps = {
    withLeadingIcon?: boolean;
};

function InputStory({
    withLeadingIcon,
    value,
    onClearInput,
    type,
    ...props
}: ComponentProps<typeof Input> & CustomStoryProps): JSX.Element {
    const [inputValue, setInputValue] = useState(value ?? '');

    useEffect(() => {
        setInputValue(value ?? '');
    }, [value]);

    return (
        <Input
            {...props}
            onChange={(e) => setInputValue(e.target.value)}
            value={inputValue}
            onClearInput={() => setInputValue('')}
            leadingIcon={withLeadingIcon ? <PlaceholderReplace /> : undefined}
            type={type}
        />
    );
}

const meta = {
    component: Input,
    tags: ['autodocs'],
} satisfies Meta<typeof Input>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
        caption: 'Caption',
        type: InputType.Text,
    },
    argTypes: {
        amountCounter: {
            control: {
                type: 'text',
            },
        },
        type: {
            control: {
                type: 'select',
                options: Object.values(InputType),
            },
        },
        onValueChange: {
            control: {
                type: 'none',
            },
        },
    },
    render: (props) => <InputStory {...props} />,
};

export const WithLeadingElement: Story = {
    args: {
        type: InputType.Text,
        placeholder: 'Placeholder',
        amountCounter: '10',
        caption: 'Caption',
    },
    render: (props) => <InputStory {...props} withLeadingIcon />,
};

export const WithMaxTrailingButton: Story = {
    args: {
        type: InputType.Number,
        placeholder: 'Send IOTAs',
        amountCounter: 'Max 10 IOTA',
        caption: 'Enter token amount',
        supportingText: 'IOTA',
        trailingElement: <PlaceholderReplace />,
    },
    render: ({ value, ...props }) => {
        const [inputValue, setInputValue] = useState<string>('');
        const [error, setError] = useState<string | undefined>();

        function onMaxClick() {
            setInputValue('10');
            checkInputValidity(inputValue);
        }

        const onChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
            setInputValue(e.target.value);
        }, []);

        function checkInputValidity(value: string) {
            if (Number(value) < 0) {
                setError('Value must be greater than 0');
            } else if (Number(value) > 10) {
                setError('Value must be less than 10');
            } else {
                setError(undefined);
            }
        }

        useEffect(() => {
            checkInputValidity(inputValue);
        }, [inputValue]);

        return (
            <Input
                {...props}
                required
                label="Send Tokens"
                value={inputValue}
                trailingElement={<ButtonPill onClick={onMaxClick}>Max</ButtonPill>}
                errorMessage={error}
                onChange={onChange}
                onClearInput={() => setInputValue('')}
            />
        );
    },
};

export const NumericFormatInput: Story = {
    args: {
        type: InputType.NumericFormat,
        placeholder: 'Enter the IOTA Amount',
        amountCounter: '10',
        caption: 'Caption',
        suffix: ' IOTA',
        prefix: '~ ',
    },
    render: (props) => {
        const [inputValue, setInputValue] = useState<string>('');

        function onMaxClick() {
            setInputValue('10');
        }

        return (
            <InputStory
                {...props}
                value={inputValue}
                trailingElement={<ButtonPill onClick={onMaxClick}>Max</ButtonPill>}
            />
        );
    },
};
