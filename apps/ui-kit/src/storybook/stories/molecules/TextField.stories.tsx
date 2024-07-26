// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { TextField, TextFieldType } from '@/components/molecules/text-field';
import { PlaceholderReplace } from '@iota/ui-icons';
import { ComponentProps, useCallback, useEffect, useState } from 'react';
import { Button } from '@/lib';

type CustomStoryProps = {
    withLeadingIcon?: boolean;
};

function TextFieldStory({
    withLeadingIcon,
    ...props
}: ComponentProps<typeof TextField> & CustomStoryProps): JSX.Element {
    const [value, setValue] = useState(props.value ?? '');

    useEffect(() => {
        setValue(props.value ?? '');
    }, [props.value]);

    return (
        <TextField
            {...props}
            onChange={(value) => setValue(value)}
            value={value}
            leadingIcon={withLeadingIcon ? <PlaceholderReplace /> : undefined}
        />
    );
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
        leadingIcon: {
            control: {
                type: 'none',
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
        supportingText: 'IOTA',
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

export const TextAreaInput: Story = {
    args: {
        type: TextFieldType.TextArea,
        placeholder: '0x...',
        caption: 'Submit your address',
    },
    render: (props) => {
        const { onChange, ...storyProps } = props;
        const [value, setValue] = useState(props.value ?? '');

        useEffect(() => {
            setValue(props.value ?? '');
        }, [props.value]);

        function onSubmit() {
            alert(value);
        }

        function handleOnChange(value: string) {
            setValue(value);
        }

        return (
            <div className="w-[300px]">
                <TextField onChange={handleOnChange} {...storyProps} isToggleButtonVisible />
                <div className="flex w-full justify-end">
                    <Button onClick={() => onSubmit()} text="Submit" />
                </div>
            </div>
        );
    },
};
