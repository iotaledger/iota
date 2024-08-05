// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { InputField, InputFieldType } from '@/lib/components/molecules/input-field';
import { PlaceholderReplace } from '@iota/ui-icons';
import { ComponentProps, useCallback, useEffect, useState } from 'react';

type CustomStoryProps = {
    withLeadingIcon?: boolean;
};

function InputFieldStory({
    withLeadingIcon,
    value,
    onClearInput,
    ...props
}: ComponentProps<typeof InputField> & CustomStoryProps): JSX.Element {
    const [inputValue, setInputValue] = useState(value ?? '');

    useEffect(() => {
        setInputValue(value ?? '');
    }, [value]);

    return (
        <InputField
            {...props}
            onChange={(value) => setInputValue(value)}
            value={inputValue}
            onClearInput={() => setInputValue('')}
            leadingIcon={withLeadingIcon ? <PlaceholderReplace /> : undefined}
        />
    );
}

const meta = {
    component: InputField,
    tags: ['autodocs'],
} satisfies Meta<typeof InputField>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
        caption: 'Caption',
        type: InputFieldType.Text,
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
                options: Object.values(InputFieldType),
            },
        },
    },
    render: (props) => <InputFieldStory {...props} />,
};

export const WithLeadingElement: Story = {
    args: {
        type: InputFieldType.Text,
        placeholder: 'Placeholder',
        amountCounter: '10',
        caption: 'Caption',
    },
    render: (props) => <InputFieldStory {...props} withLeadingIcon />,
};

export const WithMaxTrailingButton: Story = {
    args: {
        type: InputFieldType.Number,
        placeholder: 'Send IOTAs',
        amountCounter: 'Max 10 IOTA',
        caption: 'Enter token amount',
        supportingText: 'IOTA',
        trailingElement: <PlaceholderReplace />,
    },
    render: ({ value, ...props }) => {
        const [inputValue, setInputValue] = useState(value ?? '');
        const [error, setError] = useState<string | undefined>();

        useEffect(() => {
            setInputValue(value ?? '');
        }, [value]);

        function onMaxClick() {
            setInputValue('10');
        }

        const onChange = useCallback((value: string) => {
            setInputValue(value);
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

        const TrailingMaxButton = () => {
            return (
                <button
                    onClick={onMaxClick}
                    className="flex items-center justify-center rounded-xl border border-neutral-60 px-xxs py-xxxs"
                >
                    <span className="font-inter text-label-md">Max</span>
                </button>
            );
        };

        return (
            <InputField
                {...props}
                required
                label="Send Tokens"
                value={inputValue}
                trailingElement={<TrailingMaxButton />}
                errorMessage={error}
                onChange={onChange}
                onClearInput={() => setInputValue('')}
            />
        );
    },
};
