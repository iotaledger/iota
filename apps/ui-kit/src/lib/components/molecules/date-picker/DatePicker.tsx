// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from 'react';
import cx from 'classnames';
import {
    ArrowLeft,
    ArrowRight,
    Calendar,
    DoubleArrowLeft,
    DoubleArrowRight,
} from '@iota/apps-ui-icons';
import { ButtonUnstyled } from '@/components/atoms/button';
import { InputWrapper, LabelHtmlTag } from '../input/InputWrapper';
import {
    DATE_PICKER_CALENDAR_CLASSES,
    DATE_PICKER_DAY_BASE_CLASSES,
    DATE_PICKER_DAY_DISABLED_CLASSES,
    DATE_PICKER_DAY_HOVER_CLASSES,
    DATE_PICKER_DAY_OUTSIDE_MONTH_CLASSES,
    DATE_PICKER_DAY_SELECTED_CLASSES,
    DATE_PICKER_DAY_TODAY_CLASSES,
    DATE_PICKER_HEADER_BUTTON_CLASSES,
    DATE_PICKER_NAV_BUTTON_CLASSES,
    DATE_PICKER_TRIGGER_CLASSES,
    DATE_PICKER_WEEKDAY_CLASSES,
    DATE_PICKER_YEAR_CELL_BASE_CLASSES,
    DATE_PICKER_YEAR_CELL_CURRENT_CLASSES,
    DATE_PICKER_YEAR_CELL_DISABLED_CLASSES,
    DATE_PICKER_YEAR_CELL_HOVER_CLASSES,
    DATE_PICKER_YEAR_CELL_SELECTED_CLASSES,
} from './date-picker.classes';
import { DatePickerFormat } from './date-picker.enums';
import { DECADE_SIZE, YEAR_GRID_SIZE, MONTHS, WEEKDAYS } from './date-picker.constants';
import {
    startOfDay,
    decadeStart,
    isDateDisabled,
    buildCalendarGrid,
    isSameDay,
    isYearDisabled,
    formatDate,
} from './date-picker.helpers';

type CalendarView = 'days' | 'years';

export interface DatePickerProps {
    /**
     * The currently selected date value.
     */
    value?: Date;
    /**
     * Callback fired when the user selects a date.
     */
    onChange?: (date: Date) => void;
    /**
     * The earliest selectable date (inclusive).
     */
    minDate?: Date;
    /**
     * The latest selectable date (inclusive).
     */
    maxDate?: Date;
    /**
     * Controls how the selected date is displayed in the trigger field.
     */
    dateFormat?: DatePickerFormat;
    /**
     * The field label shown above the trigger.
     */
    label?: string;
    /**
     * Caption text shown below the trigger.
     */
    caption?: string;
    /**
     * Error message; overrides caption when set.
     */
    errorMessage?: string;
    /**
     * Whether the field is disabled.
     */
    disabled?: boolean;
    /**
     * Placeholder text shown when no date is selected.
     */
    placeholder?: string;
}

export function DatePicker({
    value,
    onChange,
    minDate,
    maxDate,
    dateFormat = DatePickerFormat.DayMonthYear,
    label,
    caption,
    errorMessage,
    disabled,
    placeholder = 'Select a date',
}: DatePickerProps): React.JSX.Element {
    const today = startOfDay(new Date());

    const [isOpen, setIsOpen] = useState(false);
    const [calendarView, setCalendarView] = useState<CalendarView>('days');
    const [viewYear, setViewYear] = useState(() => value?.getFullYear() ?? today.getFullYear());
    const [viewMonth, setViewMonth] = useState(() => value?.getMonth() ?? today.getMonth());

    const wrapperRef = useRef<HTMLDivElement>(null);

    useEffect(() => {
        if (value) {
            setViewYear(value.getFullYear());
            setViewMonth(value.getMonth());
        }
    }, [value]);

    useEffect(() => {
        if (!isOpen) setCalendarView('days');
    }, [isOpen]);

    useEffect(() => {
        if (!isOpen) return;
        function handleClickOutside(e: MouseEvent) {
            if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
                setIsOpen(false);
            }
        }
        document.addEventListener('mousedown', handleClickOutside);
        return () => document.removeEventListener('mousedown', handleClickOutside);
    }, [isOpen]);

    useEffect(() => {
        if (!isOpen) return;
        function handleKeyDown(e: KeyboardEvent) {
            if (e.key === 'Escape') setIsOpen(false);
        }
        document.addEventListener('keydown', handleKeyDown);
        return () => document.removeEventListener('keydown', handleKeyDown);
    }, [isOpen]);

    function toggleOpen() {
        if (!disabled) setIsOpen((prev) => !prev);
    }

    function navigatePrevMonth() {
        if (viewMonth === 0) {
            setViewMonth(11);
            setViewYear((year) => year - 1);
        } else {
            setViewMonth((month) => month - 1);
        }
    }

    function navigateNextMonth() {
        if (viewMonth === 11) {
            setViewMonth(0);
            setViewYear((year) => year + 1);
        } else {
            setViewMonth((month) => month + 1);
        }
    }

    function goBackTenYears() {
        setViewYear((year) => year - DECADE_SIZE);
    }

    function goForwardTenYears() {
        setViewYear((year) => year + DECADE_SIZE);
    }

    function goBackOneDecade() {
        setViewYear((year) => decadeStart(year) - DECADE_SIZE);
    }

    function goToNextDecade() {
        setViewYear((year) => decadeStart(year) + DECADE_SIZE);
    }

    function handleYearSelect(year: number) {
        setViewYear(year);
        setCalendarView('days');
    }

    function handleDayClick(date: Date) {
        if (isDateDisabled(date, minDate, maxDate)) return;
        onChange?.(date);
        setIsOpen(false);
    }

    const calendarGrid = buildCalendarGrid(viewYear, viewMonth);

    const decadeStartYear = decadeStart(viewYear);
    const yearsInDecade = Array.from({ length: YEAR_GRID_SIZE }, (_, i) => decadeStartYear + i);

    const canNavigateBackDecade =
        !minDate || new Date(viewYear - DECADE_SIZE, viewMonth + 1, 0) >= startOfDay(minDate);
    const canNavigateToPreviousMonth =
        !minDate || new Date(viewYear, viewMonth, 0) >= startOfDay(minDate);
    const canNavigateNextMonth =
        !maxDate || new Date(viewYear, viewMonth + 1, 1) <= startOfDay(maxDate);
    const canNavigateNextDecade =
        !maxDate || new Date(viewYear + DECADE_SIZE, viewMonth, 1) <= startOfDay(maxDate);

    const isPreviousDecadeAccessible =
        !minDate || new Date(decadeStartYear - 1, 11, 31) >= startOfDay(minDate);
    const isNextDecadeAccessible =
        !maxDate || new Date(decadeStartYear + YEAR_GRID_SIZE, 0, 1) <= startOfDay(maxDate);

    return (
        <InputWrapper
            label={label}
            caption={caption}
            disabled={disabled}
            errorMessage={errorMessage}
            labelHtmlTag={LabelHtmlTag.Div}
        >
            <div className="relative w-full" ref={wrapperRef}>
                <ButtonUnstyled
                    onClick={toggleOpen}
                    disabled={disabled}
                    aria-expanded={isOpen}
                    aria-haspopup="dialog"
                    aria-label={value ? formatDate(value, dateFormat) : placeholder}
                    className={cx(DATE_PICKER_TRIGGER_CLASSES, {
                        'date-picker-trigger-border-focus-color': isOpen,
                    })}
                >
                    <span
                        className={cx('block w-full text-start text-body-lg', {
                            'date-picker-trigger-placeholder-color': !value,
                            'date-picker-trigger-text-color': !!value,
                        })}
                    >
                        {value ? formatDate(value, dateFormat) : placeholder}
                    </span>
                    <span className="input-icon-color flex-shrink-0">
                        <Calendar />
                    </span>
                </ButtonUnstyled>

                {isOpen && (
                    <div
                        role="dialog"
                        aria-label="Date picker calendar"
                        className={cx(DATE_PICKER_CALENDAR_CLASSES, 'w-[280px]')}
                    >
                        {calendarView === 'days' ? (
                            <>
                                <div className="mb-1 flex items-center justify-between">
                                    <ButtonUnstyled
                                        onClick={goBackTenYears}
                                        disabled={!canNavigateBackDecade}
                                        aria-label="Previous 10 years"
                                        className={DATE_PICKER_NAV_BUTTON_CLASSES}
                                    >
                                        <DoubleArrowLeft />
                                    </ButtonUnstyled>

                                    <div className="flex flex-1 items-center justify-between px-1">
                                        <ButtonUnstyled
                                            onClick={navigatePrevMonth}
                                            disabled={!canNavigateToPreviousMonth}
                                            aria-label="Previous month"
                                            className={DATE_PICKER_NAV_BUTTON_CLASSES}
                                        >
                                            <ArrowLeft />
                                        </ButtonUnstyled>

                                        <ButtonUnstyled
                                            onClick={() => setCalendarView('years')}
                                            aria-label="Select year"
                                            className={DATE_PICKER_HEADER_BUTTON_CLASSES}
                                        >
                                            {MONTHS[viewMonth]} {viewYear}
                                        </ButtonUnstyled>

                                        <ButtonUnstyled
                                            onClick={navigateNextMonth}
                                            disabled={!canNavigateNextMonth}
                                            aria-label="Next month"
                                            className={DATE_PICKER_NAV_BUTTON_CLASSES}
                                        >
                                            <ArrowRight />
                                        </ButtonUnstyled>
                                    </div>

                                    <ButtonUnstyled
                                        onClick={goForwardTenYears}
                                        disabled={!canNavigateNextDecade}
                                        aria-label="Next 10 years"
                                        className={DATE_PICKER_NAV_BUTTON_CLASSES}
                                    >
                                        <DoubleArrowRight />
                                    </ButtonUnstyled>
                                </div>

                                <div className="mb-1 grid grid-cols-7">
                                    {WEEKDAYS.map((day) => (
                                        <div key={day} className={DATE_PICKER_WEEKDAY_CLASSES}>
                                            {day}
                                        </div>
                                    ))}
                                </div>

                                <div className="grid grid-cols-7">
                                    {calendarGrid.map((date, index) => {
                                        const isCurrentMonth = date.getMonth() === viewMonth;
                                        const isSelected = !!value && isSameDay(date, value);
                                        const isToday = isSameDay(date, today);
                                        const isDisabled = isDateDisabled(date, minDate, maxDate);
                                        const isOutsideMonth = !isCurrentMonth;

                                        return (
                                            <div
                                                key={index}
                                                className="flex items-center justify-center p-0.5"
                                            >
                                                <ButtonUnstyled
                                                    onClick={() => handleDayClick(date)}
                                                    disabled={isDisabled || isOutsideMonth}
                                                    aria-label={formatDate(
                                                        date,
                                                        DatePickerFormat.DayMonthYear,
                                                    )}
                                                    aria-selected={isSelected}
                                                    aria-disabled={isDisabled || isOutsideMonth}
                                                    className={cx(DATE_PICKER_DAY_BASE_CLASSES, {
                                                        [DATE_PICKER_DAY_SELECTED_CLASSES]:
                                                            isSelected,
                                                        [DATE_PICKER_DAY_TODAY_CLASSES]:
                                                            isToday && !isSelected,
                                                        [DATE_PICKER_DAY_DISABLED_CLASSES]:
                                                            isDisabled && !isOutsideMonth,
                                                        [DATE_PICKER_DAY_OUTSIDE_MONTH_CLASSES]:
                                                            isOutsideMonth,
                                                        [DATE_PICKER_DAY_HOVER_CLASSES]:
                                                            !isSelected &&
                                                            !isDisabled &&
                                                            !isOutsideMonth,
                                                    })}
                                                >
                                                    {date.getDate()}
                                                </ButtonUnstyled>
                                            </div>
                                        );
                                    })}
                                </div>
                            </>
                        ) : (
                            <>
                                <div className="mb-2 flex items-center justify-between">
                                    <ButtonUnstyled
                                        onClick={goBackOneDecade}
                                        disabled={!isPreviousDecadeAccessible}
                                        aria-label="Previous decade"
                                        className={DATE_PICKER_NAV_BUTTON_CLASSES}
                                    >
                                        <DoubleArrowLeft />
                                    </ButtonUnstyled>

                                    <span className="date-picker-header-text-color text-label-lg font-semibold">
                                        {decadeStartYear} – {decadeStartYear + YEAR_GRID_SIZE - 1}
                                    </span>

                                    <ButtonUnstyled
                                        onClick={goToNextDecade}
                                        disabled={!isNextDecadeAccessible}
                                        aria-label="Next decade"
                                        className={DATE_PICKER_NAV_BUTTON_CLASSES}
                                    >
                                        <DoubleArrowRight />
                                    </ButtonUnstyled>
                                </div>

                                <div className="grid grid-cols-3 gap-1">
                                    {yearsInDecade.map((year) => {
                                        const isSelected = value?.getFullYear() === year;
                                        const isCurrent = today.getFullYear() === year;
                                        const isDisabled = isYearDisabled(year, minDate, maxDate);

                                        return (
                                            <ButtonUnstyled
                                                key={year}
                                                onClick={() => handleYearSelect(year)}
                                                disabled={isDisabled}
                                                aria-label={String(year)}
                                                aria-selected={isSelected}
                                                className={cx(DATE_PICKER_YEAR_CELL_BASE_CLASSES, {
                                                    [DATE_PICKER_YEAR_CELL_SELECTED_CLASSES]:
                                                        isSelected,
                                                    [DATE_PICKER_YEAR_CELL_CURRENT_CLASSES]:
                                                        isCurrent && !isSelected,
                                                    [DATE_PICKER_YEAR_CELL_DISABLED_CLASSES]:
                                                        isDisabled,
                                                    [DATE_PICKER_YEAR_CELL_HOVER_CLASSES]:
                                                        !isSelected && !isDisabled,
                                                })}
                                            >
                                                {year}
                                            </ButtonUnstyled>
                                        );
                                    })}
                                </div>
                            </>
                        )}
                    </div>
                )}
            </div>
        </InputWrapper>
    );
}
