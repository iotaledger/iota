# @iota/apps-ui-kit

A UI kit library for building consistent IOTA applications.

## Installation

Install the package using your preferred package manager:

```bash
# Replace with npm, yarn, bun, etc.
pnpm install @iota/apps-ui-kit
```

## Configuration

1. Import the UI Kit global styles in your app global CSS file (e.g., `src/index.css` or `src/global.css`):

```css
@import '@iota/apps-ui-kit/styles';

@tailwind base;
@tailwind components;
@tailwind utilities;
```

2. Configure your Tailwind CSS setup to include the UI Kit styles. Update your `tailwind.config.ts` file:

```js
import { uiKitResponsivePreset } from '@iota/apps-ui-kit';
import type { Config } from 'tailwindcss';

export default {
  presets: [uiKitResponsivePreset],
  content: ['./src/**/*.{js,ts,jsx,tsx,mdx}'],
  darkMode: 'class', // configure as needed ('selector' or 'class')
  theme: {
    extend: {},
  },
} satisfies Partial<Config>;
```

3. Use components from the UI kit in your application:

```typescript
import { Button } from '@iota/apps-ui-kit';
```
