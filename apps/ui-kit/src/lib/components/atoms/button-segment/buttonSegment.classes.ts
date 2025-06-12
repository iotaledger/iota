// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export const BACKGROUND_COLORS = 'bg-transparent';
export const BACKGROUND_COLORS_SELECTED = 'button-segment-bg-color-selected';

const TEXT_COLOR = 'button-segment-text-color-default';
const TEXT_COLOR_HOVER = 'button-segment-text-color-hover';
const TEXT_COLOR_FOCUSED = 'button-segment-text-color-focused';

export const TEXT_COLORS = `${TEXT_COLOR} ${TEXT_COLOR_HOVER} ${TEXT_COLOR_FOCUSED}`;
export const TEXT_COLORS_SELECTED = 'button-segment-text-color-selected';

export const UNDERLINED = `
  button-segment-underline-base
  before:bottom-0
  button-segment-underline-hover
  button-segment-underline-hover-color
`;

export const UNDERLINED_SELECTED = `
  button-segment-underline-base
  button-segment-underline-selected
`;
