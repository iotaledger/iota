// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    TEXT_LABEL_CLASSES,
    TEXT_BODY_CLASSES,
    TEXT_BODY_DISAMBIGUOUS_CLASSES,
    TEXT_TITLE_CLASSES,
    TEXT_HEADLINE_CLASSES,
    TEXT_DISPLAY_CLASSES,
} from '@/lib/tailwind/constants';
import type { TypeSetProps } from '../blocks';

export const TYPESETS: TypeSetProps[] = [
    {
        label: 'Label',
        fontFamily: 'Inter',
        typeset: TEXT_LABEL_CLASSES,
        fontWeight: 500,
    },
    {
        label: 'Body',
        fontFamily: 'Inter',
        typeset: TEXT_BODY_CLASSES,
        fontWeight: 400,
    },
    {
        label: 'Body Disambiguous',
        fontFamily: 'Inter',
        typeset: TEXT_BODY_DISAMBIGUOUS_CLASSES,
        fontWeight: 400,
    },
    {
        label: 'Title',
        fontFamily: 'AllianceNo2',
        typeset: TEXT_TITLE_CLASSES,
        fontWeight: 500,
    },
    {
        label: 'Headline',
        fontFamily: 'AllianceNo2',
        typeset: TEXT_HEADLINE_CLASSES,
        fontWeight: 400,
    },
    {
        label: 'Display',
        fontFamily: 'AllianceNo2',
        typeset: TEXT_DISPLAY_CLASSES,
        fontWeight: 400,
    },
];
