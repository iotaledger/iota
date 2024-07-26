// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { TextField, TextFieldType } from '@/components/molecules/text-field';
import { PlaceholderReplace } from '@iota/ui-icons';
import { ComponentProps, useCallback, useEffect, useState } from 'react';

type CustomStoryProps = {
    withLeadingIcon?: boolean;
};

function TextFieldStory({
    withLeadingIcon,
    value,
    ...props
}: ComponentProps<typeof TextField> & CustomStoryProps): JSX.Element {
    const [inputValue, setInputValue] = useState(value ?? '');

    useEffect(() => {
        setInputValue(value ?? '');
    }, [value]);

    return <TextField {...props} onChange={(value) => setInputValue(value)} value={inputValue} />;
}

const meta = {
    component: TextField,
    tags: ['autodocs'],
} satisfies Meta<typeof TextField>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
        caption: 'Caption',
        type: TextFieldType.Text,
    },
    argTypes: {
        amountCounter: {
            control: {
                type: 'text',
            },
        },
    },
    render: (props) => <TextFieldStory {...props} />,
};

export const WithLeadingElement: Story = {
    args: {
        type: TextFieldType.Text,
        placeholder: 'Placeholder',
        amountCounter: '10',
        caption: 'Caption',
    },
    render: (props) => <TextFieldStory {...props} withLeadingIcon />,
};

export const WithMaxTrailingButton: Story = {
    args: {
        type: TextFieldType.Text,
        placeholder: 'Send IOTAs',
        amountCounter: 'Max 10 IOTA',
        caption: 'Enter token amount',
        trailingElement: <PlaceholderReplace />,
    },
    render: (props) => {
        const [value, setValue] = useState(props.value ?? '');
        const [error, setError] = useState<string | undefined>();

        useEffect(() => {
            setValue(props.value ?? '');
        }, [props.value]);

        function onMaxClick() {
            setValue('10');
        }

        const onChange = useCallback((value: string) => {
            setValue(value);
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
            checkInputValidity(value);
        }, [value]);

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
            <TextField
                {...props}
                required
                label="Send Tokens"
                value={value}
                trailingElement={<TrailingMaxButton />}
                errorMessage={error}
                onChange={onChange}
                onClearInput={() => setValue('')}
            />
        );
    },
};
