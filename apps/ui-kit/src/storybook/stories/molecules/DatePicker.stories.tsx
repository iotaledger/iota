// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Meta, StoryObj } from '@storybook/react';
import { useState } from 'react';
import { DatePicker, DatePickerFormat } from '@/components/molecules/date-picker';

const meta: Meta<typeof DatePicker> = {
    component: DatePicker,
    tags: ['autodocs'],
    parameters: {
        layout: 'centered',
    },
    argTypes: {
        dateFormat: {
            control: { type: 'select' },
            options: Object.values(DatePickerFormat),
        },
    },
} satisfies Meta<typeof DatePicker>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {
    args: {
        label: 'Select Date',
        caption: 'Choose a date from the calendar',
        placeholder: 'DD/MM/YYYY',
        dateFormat: DatePickerFormat.DayMonthYear,
    },
    render: (args) => {
        const [date, setDate] = useState<Date | undefined>(undefined);
        return (
            <div className="w-72 pb-80">
                <DatePicker {...args} value={date} onChange={setDate} />
            </div>
        );
    },
};

export const WithDateFormat: Story = {
    args: {
        label: 'Format DD/MM/YYYY',
        caption: 'Change the date format from the controls ',
        dateFormat: DatePickerFormat.DayMonthYear,
    },
    render: (args) => {
        const [date, setDate] = useState<Date | undefined>(new Date(2026, 3, 15));
        return (
            <div className="w-72 pb-80">
                <DatePicker {...args} value={date} onChange={setDate} />
            </div>
        );
    },
};

export const WithMinMaxLimitsInclusive: Story = {
    name: 'Min & Max (inclusive)',
    args: {
        label: 'April 2026 only',
        caption: 'Min = Apr 1 · Max = Apr 30 · both selectable',
        dateFormat: DatePickerFormat.DayMonthYear,
    },
    render: (args) => {
        const minDate = new Date(2026, 3, 1); // April 1 2026
        const maxDate = new Date(2026, 3, 30); // April 30 2026
        const [date, setDate] = useState<Date | undefined>(undefined);
        return (
            <div className="w-72 pb-80">
                <DatePicker
                    {...args}
                    value={date}
                    onChange={setDate}
                    minDate={minDate}
                    maxDate={maxDate}
                />
            </div>
        );
    },
};

export const YearPickerNavigation: Story = {
    name: 'Year picker (click "Month Year")',
    args: {
        label: 'Birth date',
        caption: 'Click "April 2026" header to jump to any year',
        dateFormat: DatePickerFormat.DayMonthYear,
    },
    render: (args) => {
        const [date, setDate] = useState<Date | undefined>(undefined);
        return (
            <div className="w-72 pb-80">
                <DatePicker {...args} value={date} onChange={setDate} />
            </div>
        );
    },
};

export const WithError: Story = {
    args: {
        label: 'Expiry Date',
        errorMessage: 'Date is required',
        placeholder: 'DD/MM/YYYY',
        dateFormat: DatePickerFormat.DayMonthYear,
    },
    render: (args) => {
        const [date, setDate] = useState<Date | undefined>(undefined);
        return (
            <div className="w-72 pb-80">
                <DatePicker {...args} value={date} onChange={setDate} />
            </div>
        );
    },
};

export const Disabled: Story = {
    args: {
        label: 'Locked Date',
        caption: 'This field is disabled',
        disabled: true,
        dateFormat: DatePickerFormat.DayMonthYear,
    },
    render: (args) => {
        return (
            <div className="w-72">
                <DatePicker {...args} value={new Date(2026, 0, 1)} />
            </div>
        );
    },
};
