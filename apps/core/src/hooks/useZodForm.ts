// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { zodResolver } from '@hookform/resolvers/zod';
import { useForm } from 'react-hook-form';
import type { UseFormProps } from 'react-hook-form';
import type { TypeOf, ZodSchema } from 'zod';

interface UseZodFormProps<T extends ZodSchema> extends UseFormProps<TypeOf<T>> {
    schema: T;
}

export const useZodForm = <T extends ZodSchema>({ schema, ...formConfig }: UseZodFormProps<T>) =>
    useForm({
        ...formConfig,
        resolver: zodResolver(schema),
    });
