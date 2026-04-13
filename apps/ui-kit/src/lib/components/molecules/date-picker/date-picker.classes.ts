// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const DATE_PICKER_TRIGGER_CLASSES =
    'date-picker-trigger-border-color focus-visible:enabled:date-picker-trigger-border-focus-color hover:enabled:date-picker-trigger-border-hover-color group-[.errored]:date-picker-trigger-border-error-color flex w-full flex-row items-center gap-x-3 rounded-lg border px-md py-sm disabled:cursor-not-allowed [&_svg]:h-5 [&_svg]:w-5';

export const DATE_PICKER_CALENDAR_CLASSES =
    'date-picker-calendar-bg date-picker-calendar-border-color absolute z-50 mt-1 rounded-lg border p-3 shadow-lg';

export const DATE_PICKER_DAY_BASE_CLASSES =
    'date-picker-day-text-color flex h-8 w-8 items-center justify-center rounded-full text-body-md transition-colors hover:delay-75';

export const DATE_PICKER_DAY_HOVER_CLASSES =
    'enabled:hover:date-picker-day-bg-hover enabled:hover:date-picker-day-text-color-hover cursor-pointer';

export const DATE_PICKER_DAY_SELECTED_CLASSES =
    'date-picker-day-bg-selected date-picker-day-text-selected cursor-pointer';

export const DATE_PICKER_DAY_TODAY_CLASSES = 'date-picker-day-today-ring font-semibold';

export const DATE_PICKER_DAY_DISABLED_CLASSES =
    'date-picker-day-text-disabled cursor-not-allowed opacity-40';

export const DATE_PICKER_DAY_OUTSIDE_MONTH_CLASSES =
    'date-picker-day-text-outside-month cursor-default opacity-30';

export const DATE_PICKER_WEEKDAY_CLASSES =
    'date-picker-weekday-text-color flex h-8 w-8 items-center justify-center text-label-sm font-medium';

export const DATE_PICKER_NAV_BUTTON_CLASSES =
    'date-picker-nav-icon-color enabled:hover:date-picker-nav-bg-hover flex h-8 w-8 items-center justify-center rounded-full transition-colors hover:delay-75 disabled:cursor-not-allowed disabled:opacity-40 [&_svg]:h-4 [&_svg]:w-4';

export const DATE_PICKER_HEADER_BUTTON_CLASSES =
    'date-picker-header-text-color enabled:hover:date-picker-nav-bg-hover rounded-md px-1 py-0.5 text-label-lg font-semibold transition-colors hover:delay-75 shrink-0';

export const DATE_PICKER_YEAR_CELL_BASE_CLASSES =
    'date-picker-day-text-color flex w-full items-center justify-center rounded-lg py-2 text-body-md transition-colors hover:delay-75';

export const DATE_PICKER_YEAR_CELL_HOVER_CLASSES = DATE_PICKER_DAY_HOVER_CLASSES;
export const DATE_PICKER_YEAR_CELL_SELECTED_CLASSES = DATE_PICKER_DAY_SELECTED_CLASSES;
export const DATE_PICKER_YEAR_CELL_CURRENT_CLASSES = DATE_PICKER_DAY_TODAY_CLASSES;
export const DATE_PICKER_YEAR_CELL_DISABLED_CLASSES = DATE_PICKER_DAY_DISABLED_CLASSES;
