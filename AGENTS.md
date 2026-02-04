# AGENTS

## Tooling

### Project structure

```
apps/
├── wallet: Browser extension web wallet
├── wallet-dashboard: Nextjs dapp dashboard
├── iota-explorer: Blockchain explorer
├── evm-bridge: EVM Bridge dapp
├── ui-kit: UI components library
├── ui-icons: Icon library
└── apps-backend: Backend services for apps

sdk/
├── typescript
├── kiosk
├── create-dapp
├── dapp-kit
├── graphql-transport
├── isc-sdk
├── ledgerjs-hw-app-iota
├── move-bytecode-template
└── wallet-standard
```

### Tools

- package manager: pnpm (workspace; requires pnpm >= 9.6.0)
- monorepo tooling: Turborepo (turbo)
- Husky for git hooks; Changesets for releases

### Quick start

- Install workspace deps: `pnpm install`
- Start a dev app: `pnpm run <app>-dev` (e.g. `pnpm run wallet-dev`)

### Common scripts (run with `pnpm run <script>`)

Note: For test, lint, and prettier scripts, you can run them for specific apps like `pnpm wallet prettier:fix`

- `test`: Run tests
- `icons`: Icons task
- `explorer`: Explorer task
- `wallet`: Wallet task
- `wallet-dashboard`: Wallet dashboard task
- `evm-bridge`: EVM bridge task
- `sdk-packages`: Build SDK packages
- `sdk`: SDK task
- `apps-backend`: Apps backend task
- `codegen`: Run codegen
- `changeset-publish`: Publish changesets
- `changeset-version`: Bump release versions
- `prettier:fix`: Fix code style
- `lint:fix`: Fix lint and format
- `prepare`: Setup git hooks
