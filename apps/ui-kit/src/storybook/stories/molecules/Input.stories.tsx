// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';

import { Input, InputType } from '@/lib/components/molecules/input';
import { CheckmarkFilled, Close, Loader2, PlaceholderReplace } from '@iota/apps-ui-icons';
import type { ComponentProps } from 'react';
import { useCallback, useEffect, useState } from 'react';
import { ButtonPill, ButtonUnstyled } from '@/lib/components/atoms/button';
import classNames from 'classnames';

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

export const NameResolverInput: Story = {
    args: {
        type: InputType.Text,
    },
    render: ({ type }) => {
        const [value, setValue] = useState<string>('');
        const { address, isLoading, error } = useNameResolver(value);

        const ICON_COLORS =
            'text-iota-primary-30 dark:text-iota-primary-70 names:text-names-primary-70';

        return (
            <Input
                type={type}
                placeholder="@name"
                label="Name Resolver"
                caption="Enter the name to resolve"
                value={value}
                onInput={(e) => setValue(e.currentTarget.value)}
                supportingValue={address}
                errorMessage={error || undefined}
                trailingElement={
                    <>
                        {value.length > 0 && (
                            <ButtonUnstyled
                                className="input-icon-color"
                                onClick={() => setValue('')}
                                tabIndex={-1}
                            >
                                <Close className="h-5 w-5" />
                            </ButtonUnstyled>
                        )}
                        {isLoading ? (
                            <Loader2 className={classNames('h-5 w-5 animate-spin', ICON_COLORS)} />
                        ) : address ? (
                            <CheckmarkFilled className={classNames('h-5 w-5', ICON_COLORS)} />
                        ) : null}
                    </>
                }
            />
        );
    },
};

function useNameResolver(name: string) {
    const [address, setAddress] = useState<string | null>(null);
    const [isLoading, setIsLoading] = useState<boolean>(true);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        setAddress(null);
        setError(null);

        if (!name) {
            setIsLoading(false);
            return;
        }

        setIsLoading(true);

        const id = setTimeout(() => {
            if (name.includes('@')) {
                setAddress(`0x3c14…187b`);
            } else {
                setError("Only names with '@' are supported");
                setAddress(null);
            }
            setIsLoading(false);
        }, 1500);

        return () => clearTimeout(id);
    }, [name]);

    return { address, isLoading, error };
}
