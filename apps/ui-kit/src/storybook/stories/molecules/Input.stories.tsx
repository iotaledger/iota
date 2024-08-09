// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Input, InputType } from '@/lib/components/molecules/input';
import { PlaceholderReplace } from '@iota/ui-icons';
import { ComponentProps, useCallback, useEffect, useState } from 'react';

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
        const [inputValue, setInputValue] = useState(value ?? '');
        const [error, setError] = useState<string | undefined>();

        useEffect(() => {
            setInputValue(value ?? '');
        }, [value]);

        function onMaxClick() {
            setInputValue('10');
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
            <Input
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
