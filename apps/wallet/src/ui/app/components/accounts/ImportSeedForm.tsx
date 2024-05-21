// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_app/shared/ButtonUI';
import { useZodForm } from '@mysten/core';
import { type SubmitHandler } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import { z } from 'zod';

import { Form } from '../../shared/forms/Form';
import { TextAreaField } from '../../shared/forms/TextAreaField';

const formSchema = z.object({});

type FormValues = z.infer<typeof formSchema>;

type ImportSeedFormProps = {
	onSubmit: SubmitHandler<FormValues>;
};

export function ImportSeedForm({ onSubmit }: ImportSeedFormProps) {
	const form = useZodForm({
		mode: 'onTouched',
		schema: formSchema,
	});
	const navigate = useNavigate();

	return (
		<Form className="flex flex-col h-full gap-2" form={form} onSubmit={onSubmit}>
			<TextAreaField label="Enter Seed" name={''} rows={3} />
			<div className="flex gap-2.5 mt-auto">
				<Button variant="outline" size="tall" text="Cancel" onClick={() => navigate(-1)} />
				<Button
					type="submit"
					disabled={true}
					variant="primary"
					size="tall"
					loading={false}
					text="Add Account"
				/>
			</div>
		</Form>
	);
}
