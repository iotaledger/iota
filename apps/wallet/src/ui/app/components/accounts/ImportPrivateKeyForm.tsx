// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useZodForm } from '@iota/core';
import { type SubmitHandler } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import { z } from 'zod';

import { privateKeyValidation } from '../../helpers/validation/privateKeyValidation';
import { Form } from '../../shared/forms/Form';
import {
    Button,
    ButtonType,
    ButtonHtmlType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
} from '@iota/apps-ui-kit';
import { TextAreaField } from '../../shared/forms/TextAreaField';
import { Exclamation } from '@iota/apps-ui-icons';

const formSchema = z.object({
    privateKey: privateKeyValidation,
});

type FormValues = z.infer<typeof formSchema>;

interface ImportPrivateKeyFormProps {
    onSubmit: SubmitHandler<FormValues>;
}

const HEXADECIMAL_KEY_MESSAGE =
    'Importing Hex encoded Private Key will soon be deprecated, please use Bech32 encoded private key that starts with "iotaprivkey" instead';

export function ImportPrivateKeyForm({ onSubmit }: ImportPrivateKeyFormProps) {
    const form = useZodForm({
        mode: 'onChange',
        schema: formSchema,
    });
    const {
        register,
        formState: { isSubmitting, isValid },
        watch,
    } = form;
    const navigate = useNavigate();
    const privateKey = watch('privateKey');
    const isHexadecimal = isValid && !privateKey.startsWith('iotaprivkey');
    return (
        <Form className="flex h-full flex-col gap-2" form={form} onSubmit={onSubmit}>
            <TextAreaField
                label="Enter Private Key"
                rows={4}
                {...register('privateKey')}
                errorMessage={form.formState.errors.privateKey?.message}
            />
            {isHexadecimal ? (
                <InfoBox
                    type={InfoBoxType.Default}
                    supportingText={HEXADECIMAL_KEY_MESSAGE}
                    icon={<Exclamation />}
                    style={InfoBoxStyle.Elevated}
                />
            ) : null}
            <div className="mt-auto flex gap-xs pt-xs">
                <Button
                    fullWidth
                    text="Cancel"
                    onClick={() => navigate(-1)}
                    type={ButtonType.Secondary}
                />
                <Button
                    htmlType={ButtonHtmlType.Submit}
                    disabled={isSubmitting || !isValid}
                    fullWidth
                    type={ButtonType.Primary}
                    text="Add Account"
                />
            </div>
        </Form>
    );
}
