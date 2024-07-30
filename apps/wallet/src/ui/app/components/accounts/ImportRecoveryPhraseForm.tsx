// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeMnemonics, validateMnemonics } from '_src/shared/utils';
import { useZodForm } from '@iota/core';
import { type SubmitHandler } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import { z } from 'zod';

import Alert from '../alert';
import { TextField, TextFieldType, Button, ButtonType } from '@iota/apps-ui-kit';

const RECOVERY_PHRASE_WORD_COUNT = 24;

const formSchema = z.object({
    recoveryPhrase: z
        .array(z.string().trim())
        .length(RECOVERY_PHRASE_WORD_COUNT)
        .transform((recoveryPhrase) => normalizeMnemonics(recoveryPhrase.join(' ')).split(' '))
        .refine((recoveryPhrase) => validateMnemonics(recoveryPhrase.join(' ')), {
            message: 'Recovery Passphrase is invalid',
        }),
});

export type FormValues = z.infer<typeof formSchema>;

interface ImportRecoveryPhraseFormProps {
    submitButtonText: string;
    cancelButtonText?: string;
    onSubmit: SubmitHandler<FormValues>;
    isTextVisible?: boolean;
}

export function ImportRecoveryPhraseForm({
    submitButtonText,
    cancelButtonText,
    onSubmit,
    isTextVisible,
}: ImportRecoveryPhraseFormProps) {
    const {
        register,
        formState: { errors, isSubmitting, isValid, touchedFields },
        handleSubmit,
        setValue,
        getValues,
        trigger,
    } = useZodForm({
        mode: 'all',
        reValidateMode: 'onChange',
        schema: formSchema,
        defaultValues: {
            recoveryPhrase: Array.from({ length: RECOVERY_PHRASE_WORD_COUNT }, () => ''),
        },
    });
    const navigate = useNavigate();
    const recoveryPhrase = getValues('recoveryPhrase');

    async function handlePaste(e: React.ClipboardEvent<HTMLInputElement>) {
        const inputText = e.clipboardData.getData('text');
        const words = inputText
            .trim()
            .split(/\W/)
            .map((aWord) => aWord.trim())
            .filter(String);

        if (words.length > 1) {
            e.preventDefault();
            const newRecoveryPhrase = [...recoveryPhrase];
            newRecoveryPhrase.splice(
                0,
                words.length,
                ...words.slice(0, RECOVERY_PHRASE_WORD_COUNT),
            );
            setValue('recoveryPhrase', newRecoveryPhrase);
            trigger('recoveryPhrase');
        }
    }

    function handleKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
        if (e.key === ' ') {
            e.preventDefault();
            const nextInput = document.getElementsByName(
                `recoveryPhrase.${recoveryPhrase.findIndex((word) => !word)}`,
            )[0];
            nextInput?.focus();
        }
    }

    return (
        <form
            className="relative flex h-full flex-col justify-between"
            onSubmit={handleSubmit(onSubmit)}
        >
            <div className="grid grid-cols-2 gap-x-2 gap-y-2.5 pb-md">
                {recoveryPhrase.map((_, index) => {
                    const recoveryPhraseId = `recoveryPhrase.${index}` as const;
                    return (
                        <TextField
                            key={recoveryPhraseId}
                            supportingText={String(index + 1)}
                            type={TextFieldType.Password}
                            isVisibilityToggleEnabled={false}
                            disabled={isSubmitting}
                            placeholder="Word"
                            isContentVisible={isTextVisible}
                            onKeyDown={handleKeyDown}
                            onPaste={handlePaste}
                            {...register(recoveryPhraseId)}
                        />
                    );
                })}
            </div>

            <div className="sticky bottom-0 left-0 flex flex-col gap-2.5 bg-neutral-100 py-sm">
                {touchedFields.recoveryPhrase && errors.recoveryPhrase && (
                    <Alert>{errors.recoveryPhrase.message}</Alert>
                )}
                <div className="flex flex-row justify-stretch gap-2.5">
                    {cancelButtonText ? (
                        <Button
                            type={ButtonType.Secondary}
                            text={cancelButtonText}
                            onClick={() => navigate(-1)}
                            fullWidth
                        />
                    ) : null}
                    <Button
                        type={ButtonType.Primary}
                        disabled={isSubmitting || isSubmitting || !isValid}
                        text={submitButtonText}
                        fullWidth
                        onClick={handleSubmit(onSubmit)}
                    />
                </div>
            </div>
        </form>
    );
}
