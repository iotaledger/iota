// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
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
    Select,
    SelectSize,
} from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

const MNEMONIC_LENGTHS = [12, 24] as const;

type MnemonicLength = (typeof MNEMONIC_LENGTHS)[number];

const formSchema = (mnemonicLength: MnemonicLength) =>
    z.object({
        recoveryPhrase: z
            .array(z.string().trim().min(1))
            .min(mnemonicLength)
            .max(mnemonicLength)
            .transform((recoveryPhrase) => normalizeMnemonics(recoveryPhrase.join(' ')).split(' '))
            .refine((recoveryPhrase) => validateMnemonics(recoveryPhrase.join(' ')), {
                message: 'Mnemonic is invalid',
            }),
    });

type FormValues = z.infer<ReturnType<typeof formSchema>>;

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
    const [mnemonicLength, setMnemonicLength] = useState<MnemonicLength>(24);
    const {
        register,
        formState: { errors, isSubmitting, isValid },
        handleSubmit,
        setValue,
        getValues,
        trigger,
        reset,
    } = useZodForm({
        mode: 'all',
        reValidateMode: 'onChange',
        schema: formSchema(mnemonicLength),
        defaultValues: {
            recoveryPhrase: Array.from({ length: mnemonicLength }, () => ''),
        },
    });
    const navigate = useNavigate();
    const recoveryPhrase = getValues('recoveryPhrase');

    function handleWordCountChange(value: string) {
        const newWordCount = Number(value) as MnemonicLength;
        setMnemonicLength(newWordCount);
        reset({ recoveryPhrase: Array.from({ length: newWordCount }, () => '') });
    }

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
        trigger('recoveryPhrase');
    }

    const errorMessage = errors?.recoveryPhrase?.root?.message || errors?.recoveryPhrase?.message;

    return (
        <form
            className="relative flex h-full flex-col justify-between"
            onSubmit={handleSubmit(onSubmit)}
        >
            <div className="flex h-full min-h-0 flex-grow flex-col gap-y-sm">
                <Select
                    value={String(mnemonicLength)}
                    onValueChange={handleWordCountChange}
                    options={MNEMONIC_LENGTHS.map((count) => ({
                        id: String(count),
                        label: `${count} words`,
                    }))}
                    size={SelectSize.Small}
                />
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
            </div>
            <div className="sticky bottom-0 left-0 flex flex-col gap-2.5 bg-neutral-100 pt-sm dark:bg-neutral-6">
                {errorMessage && recoveryPhrase.every((word) => word.length > 0) ? (
                    <InfoBox
                        type={InfoBoxType.Error}
                        supportingText={errorMessage}
                        icon={<Warning />}
                        style={InfoBoxStyle.Elevated}
                    />
                ) : null}
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
