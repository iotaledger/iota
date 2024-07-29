// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { TextArea } from '@/components/molecules/text-field';
import { useEffect, useState } from 'react';
import { Button } from '@/lib';

const meta = {
    component: TextArea,
    tags: ['autodocs'],
} satisfies Meta<typeof TextArea>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Label',
        caption: 'Caption',
    },
    argTypes: {
        amountCounter: {
            control: {
                type: 'text',
            },
        },
        cols: {
            control: {
                type: 'number',
            },
        },
        rows: {
            control: {
                type: 'number',
            },
        },
        maxLength: {
            control: {
                type: 'number',
            },
        },
        minLength: {
            control: {
                type: 'number',
            },
        },
        autoFocus: {
            control: {
                type: 'boolean',
            },
        },
    },
    render: (props) => {
        const { onChange, value, ...storyProps } = props;
        const [inputValue, setInputValue] = useState(props.value ?? '');

        useEffect(() => {
            setInputValue(props.value ?? '');
        }, [props.value]);

        function onSubmit() {
            alert(inputValue);
        }

        function handleOnChange(value: string) {
            setInputValue(value);
        }

        return (
            <div className="w-[800px]">
                <TextArea
                    onChange={handleOnChange}
                    value={inputValue}
                    isVisibilityToggleEnabled
                    {...storyProps}
                />
                <div className="flex w-full justify-end">
                    <Button onClick={() => onSubmit()} text="Submit" />
                </div>
            </div>
        );
    },
};
