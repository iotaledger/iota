// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DECADE_SIZE } from './date-picker.constants';
import { DatePickerFormat } from './date-picker.enums';

export function isSameDay(a: Date, b: Date): boolean {
    return (
        a.getFullYear() === b.getFullYear() &&
        a.getMonth() === b.getMonth() &&
        a.getDate() === b.getDate()
    );
}

export function startOfDay(date: Date): Date {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate());
}

/** Returns true when date falls outside the [min, max] inclusive range. */
export function isDateDisabled(date: Date, min?: Date, max?: Date): boolean {
    const d = startOfDay(date);
    if (min && d < startOfDay(min)) return true;
    if (max && d > startOfDay(max)) return true;
    return false;
}

/** Returns true when every day in year is outside [min, max]. */
export function isYearDisabled(year: number, min?: Date, max?: Date): boolean {
    if (min && new Date(year, 11, 31) < startOfDay(min)) return true;
    if (max && new Date(year, 0, 1) > startOfDay(max)) return true;
    return false;
}

export function formatDate(date: Date, format: DatePickerFormat): string {
    const year = String(date.getFullYear());
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const day = String(date.getDate()).padStart(2, '0');
    switch (format) {
        case DatePickerFormat.DayMonthYear:
            return `${day}/${month}/${year}`;
        case DatePickerFormat.MonthDayYear:
            return `${month}/${day}/${year}`;
        case DatePickerFormat.YearMonthDay:
            return `${year}/${month}/${day}`;
        default:
            return `${day}/${month}/${year}`;
    }
}

/**
 * Builds the 6×7 grid of dates for the calendar day view
 * starts on Sunday of the week containing the 1st of the month.
 */
export function buildCalendarGrid(year: number, month: number): Date[] {
    const firstDay = new Date(year, month, 1);
    const startOffset = firstDay.getDay(); // 0 = Sunday
    const grid: Date[] = [];
    for (let i = -startOffset; i < 42 - startOffset; i++) {
        grid.push(new Date(year, month, 1 + i));
    }
    return grid;
}

/** Returns the first year of the decade that contains year */
export function decadeStart(year: number): number {
    return Math.floor(year / DECADE_SIZE) * DECADE_SIZE;
}
