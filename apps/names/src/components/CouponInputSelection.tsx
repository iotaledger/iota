// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Close } from '@iota/apps-ui-icons';
import { ButtonUnstyled, Chip, ChipSize, ChipType, Input, InputType } from '@iota/apps-ui-kit';
import clsx from 'clsx';
import { useState } from 'react';

import type { UserSetCoupon } from './dialogs';

interface CouponInputSelectionProps {
    coupons: UserSetCoupon[];
    disabled: boolean;
    onAddCoupon: (code: string) => Promise<void>;
}

export function CouponInputSelection({
    coupons,
    disabled,
    onAddCoupon,
}: CouponInputSelectionProps) {
    const [coupon, setCoupon] = useState<string>('');

    async function addCoupon() {
        const trimmedCoupon = coupon.trim();
        if (!trimmedCoupon || coupons.some((c) => c.code === trimmedCoupon)) return;
        await onAddCoupon(trimmedCoupon);
        setCoupon('');
    }

    return (
        <div className={clsx('flex flex-col', coupons.length && 'gap-y-sm')}>
            <div className="flex flex-wrap gap-x-xs gap-y-xs">
                {coupons.map(({ code: coupon, isInvalid: isError }) => (
                    <Chip
                        data-amp-mask
                        key={coupon}
                        label={coupon}
                        onClick={() => onAddCoupon(coupon)}
                        trailingElement={<Close />}
                        size={ChipSize.Small}
                        type={isError ? ChipType.Error : undefined}
                        aria-label="Remove coupon code"
                    />
                ))}
            </div>

            <div className="flex flex-col items-start gap-y-sm">
                {coupons.length === 0 && (
                    <>
                        <Input
                            placeholder={'Have a discount code?'}
                            type={InputType.Text}
                            value={coupon}
                            onChange={(e) => setCoupon(e.target.value)}
                            onKeyDown={(e) => e.key === 'Enter' && addCoupon()}
                            onClearInput={() => setCoupon('')}
                            disabled={disabled}
                            data-amp-mask
                        />
                        <ButtonUnstyled
                            className="bg-names-gradient-primary bg-clip-text text-transparent bg-[length:200%] enabled:transition-[background-position] enabled:duration-500 enabled:hover:bg-[100%] text-label-md cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
                            onClick={addCoupon}
                            disabled={disabled}
                        >
                            + Apply Coupon
                        </ButtonUnstyled>
                    </>
                )}
            </div>
        </div>
    );
}
