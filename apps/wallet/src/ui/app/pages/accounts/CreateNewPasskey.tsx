// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate } from 'react-router-dom';

import {
    AccountsFormType,
    PageTemplate,
    useAccountsFormContext,
    useBootstrapSourceFlow,
} from '_components';
import {
    Button,
    ButtonHtmlType,
    ButtonType,
    Input,
    InputType,
    RadioButton,
} from '@iota/apps-ui-kit';
import { useZodForm } from '@iota/core';
import { z } from 'zod';
import { Form } from '../../shared/forms/Form';
import React, { useState } from 'react';

const formSchema = z.object({
    username: z.string().min(1, 'Username is required').max(50, 'Username is too long'),
});
type ImportPasskeyFormValues = z.infer<typeof formSchema>;

export function CreateNewPasskey() {
    const navigate = useNavigate();
    useBootstrapSourceFlow();

    const [authenticatorAttachment, setAuthenticatorAttachment] =
        useState<AuthenticatorAttachment>('cross-platform');
    const [, setAccountsFormValues] = useAccountsFormContext();

    const form = useZodForm({
        mode: 'onChange',
        schema: formSchema,
        defaultValues: {
            username: '',
        },
    });

    const {
        register,
        formState: { isSubmitting, isValid, errors },
    } = form;

    const handleSubmit = async (values: ImportPasskeyFormValues) => {
        setAccountsFormValues({
            type: AccountsFormType.Passkey,
            authenticatorAttachment,
            username: values.username,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.Passkey,
            }).toString()}`,
        );
    };

    const RADIO_BUTTONS: React.ComponentProps<typeof RadioButton>[] = [
        {
            label: 'Externally',
            name: 'cross-platform',
            supportingLabel: '(Recommended)',
            body: 'On your phone via a password manager (e.g. Apple / Google Passwords, LastPass) or on a hardware security key (e.g., YubiKey).',
            isChecked: authenticatorAttachment === 'cross-platform',
            onChange: () => setAuthenticatorAttachment('cross-platform'),
        },
        {
            label: 'Locally',
            name: 'platform',
            body: 'On this device using its built-in password manager.',
            isChecked: authenticatorAttachment === 'platform',
            onChange: () => setAuthenticatorAttachment('platform'),
        },
    ];

    return (
        <PageTemplate
            title="Create Passkey Account"
            isTitleCentered
            showBackButton
            onBack={() => navigate('/accounts/import-existing')}
        >
            <Form
                className="flex h-full flex-col justify-between"
                form={form}
                onSubmit={handleSubmit}
            >
                <div className="flex flex-col gap-6">
                    <Input
                        autoFocus
                        type={InputType.Text}
                        label="Account Nickname"
                        placeholder="Give your account a name"
                        errorMessage={errors.username?.message}
                        {...register('username', { shouldUnregister: true })}
                        name="username"
                        data-testid="username-input"
                    />

                    <div className="flex flex-col gap-md text-start">
                        <p className="pt-xxs text-label-md text-iota-neutral-30 dark:text-iota-neutral-80">
                            How would you like to store your passkey?
                        </p>

                        {RADIO_BUTTONS.map((radio) => (
                            <div key={radio.label} data-testid={`passkey-radio-${radio.name}`}>
                                <RadioButton {...radio} />
                            </div>
                        ))}
                    </div>
                </div>

                <div className="flex flex-col gap-4 pt-xxxs">
                    <div className="flex flex-row justify-stretch gap-2.5">
                        <Button
                            type={ButtonType.Secondary}
                            text="Cancel"
                            onClick={() => navigate('/accounts/manage')}
                            fullWidth
                        />
                        <Button
                            type={ButtonType.Primary}
                            disabled={isSubmitting || !isValid}
                            text="Continue"
                            fullWidth
                            htmlType={ButtonHtmlType.Submit}
                        />
                    </div>
                </div>
            </Form>
        </PageTemplate>
    );
}
