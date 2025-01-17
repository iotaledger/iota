// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeMnemonics, validateMnemonics } from '_src/shared/utils';
import { useZodForm } from '@iota/core';
import { type SubmitHandler } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import { z } from 'zod';

import {
    Input,
    InputType,
    Button,
    ButtonType,
    ButtonHtmlType,
    InfoBox,
    InfoBoxType,
    InfoBoxStyle,
} from '@iota/apps-ui-kit';
import { Exclamation } from '@iota/apps-ui-icons';

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

type FormValues = z.infer<typeof formSchema>;

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

    async function handlePaste(e: React.ClipboardEvent<HTMLInputElement>, index: number) {
        const inputText = e.clipboardData.getData('text');
        const words = inputText
            .trim()
            .split(/\W/)
            .map((aWord) => aWord.trim())
            .filter(String);

        if (words.length > 1) {
            e.preventDefault();
            const pasteIndex = words.length === recoveryPhrase.length ? 0 : index;
            const wordsToPaste = words.slice(0, recoveryPhrase.length - pasteIndex);
            const newRecoveryPhrase = [...recoveryPhrase];
            newRecoveryPhrase.splice(
                pasteIndex,
                wordsToPaste.length,
                ...words.slice(0, recoveryPhrase.length - pasteIndex),
            );
            setValue('recoveryPhrase', newRecoveryPhrase);
            trigger('recoveryPhrase');
        }
    }

    function handleInputKeyDown(e: React.KeyboardEvent<HTMLInputElement>) {
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
            <div className="grid grid-cols-2 gap-2 overflow-auto pb-md">
                {recoveryPhrase.map((_, index) => {
                    const recoveryPhraseId = `recoveryPhrase.${index}` as const;
                    return (
                        <Input
                            key={recoveryPhraseId}
                            supportingText={String(index + 1)}
                            type={InputType.Password}
                            isVisibilityToggleEnabled={false}
                            disabled={isSubmitting}
                            placeholder="Word"
                            isContentVisible={isTextVisible}
                            onKeyDown={handleInputKeyDown}
                            onPaste={(e) => handlePaste(e, index)}
                            id={recoveryPhraseId}
                            {...register(recoveryPhraseId)}
                        />
                    );
                })}
            </div>

            <div className="sticky bottom-0 left-0 flex flex-col gap-2.5 bg-neutral-100 pt-sm dark:bg-neutral-6">
                {touchedFields.recoveryPhrase && errors.recoveryPhrase && (
                    <InfoBox
                        type={InfoBoxType.Default}
                        supportingText={errors.recoveryPhrase.message}
                        icon={<Exclamation />}
                        style={InfoBoxStyle.Elevated}
                    />
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
                        disabled={isSubmitting || !isValid}
                        text={submitButtonText}
                        fullWidth
                        htmlType={ButtonHtmlType.Submit}
                    />
                </div>
            </div>
        </form>
    );
}
