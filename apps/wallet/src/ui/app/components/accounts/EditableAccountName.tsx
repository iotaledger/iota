// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useZodForm } from '@iota/core';
import { forwardRef, useRef, type ComponentProps } from 'react';
import toast from 'react-hot-toast';
import { z } from 'zod';

import { useBackgroundClient } from '../../hooks/useBackgroundClient';
import { Form } from '../../shared/forms/Form';

type InputProps = Omit<ComponentProps<'input'>, 'className'>;

export const Input = forwardRef<HTMLInputElement, InputProps>((props, forwardedRef) => (
    <input
        className="peer items-center rounded-sm border-none bg-transparent p-0 text-pBody font-semibold text-steel-darker outline-none transition hover:text-hero focus:bg-transparent"
        ref={forwardedRef}
        {...props}
    />
));

const formSchema = z.object({
    nickname: z.string().trim(),
});

export function EditableAccountName({ accountID, name }: { accountID: string; name: string }) {
    const backgroundClient = useBackgroundClient();
    const form = useZodForm({
        mode: 'all',
        schema: formSchema,
        values: {
            nickname: name,
        },
    });
    const { register } = form;
    const onSubmit = async ({ nickname }: { nickname: string }) => {
        if (accountID) {
            try {
                await backgroundClient.setAccountNickname({
                    id: accountID,
                    nickname: nickname || null,
                });
                const activeElement = document.activeElement as HTMLElement;
                activeElement?.blur();
            } catch (e) {
                toast.error((e as Error).message || 'Failed to set nickname');
            }
        }
    };
    const { ref, ...inputFormData } = register('nickname');
    const inputRef = useRef<HTMLInputElement | null>();
    return (
        <div>
            <Form className="flex flex-col" form={form} onSubmit={onSubmit}>
                <Input
                    {...inputFormData}
                    ref={(instance) => {
                        ref(instance);
                        inputRef.current = instance;
                    }}
                    onBlur={form.handleSubmit(onSubmit)}
                    onFocus={() => inputRef.current?.select()}
                />
                <button className="hidden" type="submit" />
            </Form>
        </div>
    );
}
