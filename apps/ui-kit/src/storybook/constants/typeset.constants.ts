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
        typeset: TEXT_LABEL_CLASSES,
    },
    {
        label: 'Body',
        typeset: TEXT_BODY_CLASSES,
    },
    {
        label: 'Body Disambiguous',
        typeset: TEXT_BODY_DISAMBIGUOUS_CLASSES,
    },
    {
        label: 'Title',
        typeset: TEXT_TITLE_CLASSES,
    },
    {
        label: 'Headline',
        typeset: TEXT_HEADLINE_CLASSES,
    },
    {
        label: 'Display',
        typeset: TEXT_DISPLAY_CLASSES,
    },
];
