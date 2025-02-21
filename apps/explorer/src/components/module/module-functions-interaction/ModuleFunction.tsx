// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useZodForm } from '@iota/core';
import {
    ConnectModal,
    useCurrentAccount,
    useIotaClient,
    useSignAndExecuteTransaction,
} from '@iota/dapp-kit';
import {
    getPureBcsSchema,
    normalizedTypeToMoveTypeSignature,
    Transaction,
} from '@iota/iota-sdk/transactions';
import { useMutation } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useWatch } from 'react-hook-form';
import { z } from 'zod';

import { useFunctionParamsDetails, useFunctionTypeArguments } from '~/hooks';
import { FunctionExecutionResult } from './FunctionExecutionResult';

import type { IotaMoveNormalizedFunction } from '@iota/iota-sdk/client';
import type { TypeOf } from 'zod';
import {
    Accordion,
    AccordionContent,
    AccordionHeader,
    Input,
    InputType,
    Title,
    Button,
    ButtonType,
    ButtonHtmlType,
    TitleSize,
} from '@iota/apps-ui-kit';

const argsSchema = z.object({
    params: z.optional(z.array(z.string().trim().min(1))),
    types: z.optional(z.array(z.string().trim().min(1))),
});

type ModuleFunctionProps = {
    packageId: string;
    moduleName: string;
    functionName: string;
    functionDetails: IotaMoveNormalizedFunction;
    defaultOpen?: boolean;
};

export function ModuleFunction({
    defaultOpen,
    packageId,
    moduleName,
    functionName,
    functionDetails,
}: ModuleFunctionProps): JSX.Element {
    const currentAccount = useCurrentAccount();
    const iotaClient = useIotaClient();
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction({
        execute: async ({ bytes, signature }) =>
            await iotaClient.executeTransactionBlock({
                transactionBlock: bytes,
                signature,
                options: {
                    showRawEffects: true,
                    showEffects: true,
                    showEvents: true,
                    showInput: true,
                },
            }),
    });

    const { handleSubmit, formState, register, control } = useZodForm({
        schema: argsSchema,
    });
    const { isValidating, isValid, isSubmitting } = formState;

    const typeArguments = useFunctionTypeArguments(functionDetails.typeParameters);
    const formTypeInputs = useWatch({ control, name: 'types' });
    const resolvedTypeArguments = useMemo(
        () => typeArguments.map((aType, index) => formTypeInputs?.[index] || aType),
        [typeArguments, formTypeInputs],
    );
    const paramsDetails = useFunctionParamsDetails({
        params: functionDetails.parameters,
        functionTypeArgNames: resolvedTypeArguments,
    });

    const execute = useMutation({
        mutationFn: async ({ params, types }: TypeOf<typeof argsSchema>) => {
            const tx = new Transaction();
            tx.moveCall({
                target: `${packageId}::${moduleName}::${functionName}`,
                typeArguments: types ?? [],
                arguments:
                    params?.map((param, i) => {
                        const moveTypeSignature = normalizedTypeToMoveTypeSignature(
                            functionDetails.parameters[i],
                        );

                        const pureBcsSchema = getPureBcsSchema(moveTypeSignature.body);

                        return pureBcsSchema ? pureBcsSchema.serialize(param) : tx.object(param);
                    }) ?? [],
            });

            const result = await signAndExecuteTransaction({ transaction: tx });

            if (result.effects?.status.status === 'failure') {
                throw new Error(result.effects.status.error || 'Transaction failed');
            }

            return result;
        },
    });

    const isExecuteDisabled = isValidating || !isValid || isSubmitting || !currentAccount;

    const [isExpanded, setIsExpanded] = useState<boolean>(defaultOpen ?? false);

    function onToggle() {
        setIsExpanded((prev) => !prev);
    }

    return (
        <Accordion>
            <AccordionHeader isExpanded={isExpanded} onToggle={onToggle}>
                <Title size={TitleSize.Small} title={functionName} />
            </AccordionHeader>
            <AccordionContent isExpanded={isExpanded}>
                <form
                    className="flex flex-col flex-nowrap items-stretch gap-md p-md--rs"
                    onSubmit={handleSubmit((formData) =>
                        execute.mutateAsync(formData).catch(() => {
                            /* ignore tx execution errors */
                        }),
                    )}
                    autoComplete="off"
                >
                    {typeArguments.map((aTypeArgument, index) => (
                        <Input
                            type={InputType.Text}
                            key={index}
                            label={`Type${index}`}
                            {...register(`types.${index}` as const)}
                            placeholder={aTypeArgument}
                        />
                    ))}
                    {paramsDetails.map(({ paramTypeText }, index) => (
                        <Input
                            type={InputType.Text}
                            key={index}
                            label={`Arg${index}`}
                            {...register(`params.${index}` as const)}
                            placeholder={paramTypeText}
                            disabled={isSubmitting}
                        />
                    ))}
                    <div className="flex items-stretch justify-end gap-1.5">
                        <Button
                            type={ButtonType.Primary}
                            htmlType={ButtonHtmlType.Submit}
                            disabled={isExecuteDisabled || execute.isPending}
                            text="Execute"
                        />
                        {currentAccount ? null : (
                            <ConnectModal
                                trigger={<Button text="Connect Wallet" type={ButtonType.Primary} />}
                            />
                        )}
                    </div>
                    {execute.error || execute.data ? (
                        <FunctionExecutionResult
                            error={
                                execute.error ? (execute.error as Error).message || 'Error' : false
                            }
                            result={execute.data || null}
                            onClear={() => {
                                execute.reset();
                            }}
                        />
                    ) : null}
                </form>
            </AccordionContent>
        </Accordion>
    );
}
