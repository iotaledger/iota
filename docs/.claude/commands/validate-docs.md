Validate the documentation build and report any errors.

## Steps

1. Run the production build from the **repo root**:
   ```sh
   pnpm iota-docs build
   ```

2. Parse the output and group errors by type:
   - **MDX parse errors** — syntax issues in `.mdx` files
   - **Broken links** — internal links pointing to missing pages or anchors
   - **Missing file references** — `file=<rootDir>/...` paths that don't resolve
   - **Missing external references** — `reference` URLs that failed to download

3. For each error, report:
   - The file and line number
   - The error message
   - A suggested fix where possible

4. If the build passes with no errors, confirm success and note any warnings.

## Notes

- The build downloads external references first; network errors are expected in offline environments — flag them separately.
- `pnpm iota-docs dev` gives faster feedback during iterative editing but does not catch all broken-link errors that `pnpm iota-docs build` catches.
