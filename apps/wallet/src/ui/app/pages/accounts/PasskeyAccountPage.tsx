// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNavigate, useSearchParams } from 'react-router-dom';

import { AccountsFormType, PageTemplate, useAccountsFormContext } from '_components';
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

export function PasskeyAccountPage() {
    const navigate = useNavigate();
    const [authenticatorAttachment, setAuthenticatorAttachment] =
        useState<AuthenticatorAttachment>('cross-platform');
    const [, setAccountsFormValues] = useAccountsFormContext();
    const [searchParams] = useSearchParams();
    const flowType = searchParams.get('flowType');
    const isCreateFlow = flowType === 'create';

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
            authenticatorAttachment: isCreateFlow ? authenticatorAttachment : undefined,
            username: values.username,
            isRestoreAccount: !isCreateFlow,
        });
        navigate(
            `/accounts/protect-account?${new URLSearchParams({
                accountsFormType: AccountsFormType.Passkey,
            }).toString()}`,
        );
    };

    const RADIO_BUTTONS: React.ComponentProps<typeof RadioButton>[] = [
        {
            label: 'Cross-Platform',
            name: 'cross-platform',
            supportingLabel: '(Recommended)',
            body: 'Use a passkey saved in your phone (Google or Apple account) or store it in a hardware key like a YubiKey.',
            isChecked: authenticatorAttachment === 'cross-platform',
            onChange: () => setAuthenticatorAttachment('cross-platform'),
        },
        {
            label: 'Platform',
            name: 'platform',
            body: 'Store a passkey on this device. Use built-in Face ID, Touch ID, or Windows Hello.',
            isChecked: authenticatorAttachment === 'platform',
            onChange: () => setAuthenticatorAttachment('platform'),
        },
    ];

    return (
        <PageTemplate
            title={`${isCreateFlow ? 'Create' : 'Import'} Passkey Account`}
            isTitleCentered
            showBackButton
            onBack={() => {
                navigate('/accounts/manage');
            }}
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
                        placeholder="Enter a nickname"
                        errorMessage={errors.username?.message}
                        {...register('username', { shouldUnregister: true })}
                        name="username"
                        data-testid="username-input"
                    />

                    {isCreateFlow && (
                        <div className="flex flex-col gap-md text-start">
                            <p className="pt-xxs text-label-md text-iota-neutral-30 dark:text-iota-neutral-80">
                                Passkey Storage and Access
                            </p>

                            {RADIO_BUTTONS.map((radio) => (
                                <div key={radio.label} data-testId={`passkey-radio-${radio.name}`}>
                                    <RadioButton {...radio} />
                                </div>
                            ))}
                        </div>
                    )}
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
                            text={isCreateFlow ? 'Create' : 'Restore'}
                            fullWidth
                            htmlType={ButtonHtmlType.Submit}
                        />
                    </div>
                </div>
            </Form>
        </PageTemplate>
    );
}
