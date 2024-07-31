// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_app/shared/ButtonUI';
import { normalizeMnemonics, validateMnemonics } from '_src/shared/utils';
import { PasswordInput } from '_src/ui/app/shared/forms/controls/PasswordInput';
import { Text } from '_src/ui/app/shared/text';
import { useZodForm } from '@iota/core';
import { type SubmitHandler } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import { z } from 'zod';

import Alert from '../alert';

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
}

export function ImportRecoveryPhraseForm({
    submitButtonText,
    cancelButtonText,
    onSubmit,
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

    return (
        <form
            className="relative flex h-full flex-col justify-between"
            onSubmit={handleSubmit(onSubmit)}
        >
            <div className="mb-4 grid grid-cols-2 gap-x-2 gap-y-2.5">
                {recoveryPhrase.map((_, index) => {
                    const recoveryPhraseId = `recoveryPhrase.${index}` as const;
                    return (
                        <label key={index} className="flex flex-col items-center gap-1.5">
                            <Text variant="captionSmall" weight="medium" color="steel-darker">
                                {index + 1}
                            </Text>
                            <PasswordInput
                                disabled={isSubmitting}
                                onKeyDown={(e) => {
                                    if (e.key === ' ') {
                                        e.preventDefault();
                                        const nextInput = document.getElementsByName(
                                            `recoveryPhrase.${index + 1}`,
                                        )[0];
                                        nextInput?.focus();
                                    }
                                }}
                                onPaste={async (e) => {
                                    const inputText = e.clipboardData.getData('text');
                                    const words = inputText
                                        .trim()
                                        .split(/\W/)
                                        .map((aWord) => aWord.trim())
                                        .filter(String);

                                    if (words.length > 1) {
                                        e.preventDefault();
                                        const pasteIndex =
                                            words.length === recoveryPhrase.length ? 0 : index;
                                        const wordsToPaste = words.slice(
                                            0,
                                            recoveryPhrase.length - pasteIndex,
                                        );
                                        const newRecoveryPhrase = [...recoveryPhrase];
                                        newRecoveryPhrase.splice(
                                            pasteIndex,
                                            wordsToPaste.length,
                                            ...words.slice(0, recoveryPhrase.length - pasteIndex),
                                        );
                                        setValue('recoveryPhrase', newRecoveryPhrase);
                                        trigger('recoveryPhrase');
                                    }
                                }}
                                id={recoveryPhraseId}
                                {...register(recoveryPhraseId)}
                            />
                        </label>
                    );
                })}
            </div>
            <div className="bg-iota-lightest sticky -bottom-7.5 -mx-6 -mb-7.5 flex flex-col gap-2.5 px-6 pb-7.5 pt-3">
                {touchedFields.recoveryPhrase && errors.recoveryPhrase && (
                    <Alert>{errors.recoveryPhrase.message}</Alert>
                )}
                <div className="flex gap-2.5">
                    {cancelButtonText ? (
                        <Button
                            variant="outline"
                            size="tall"
                            text={cancelButtonText}
                            onClick={() => navigate(-1)}
                        />
                    ) : null}
                    <Button
                        type="submit"
                        disabled={isSubmitting || !isValid}
                        variant="primary"
                        size="tall"
                        loading={isSubmitting}
                        text={submitButtonText}
                    />
                </div>
            </div>
        </form>
    );
}
