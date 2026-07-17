Create a new documentation page following the project conventions in CLAUDE.md.

## Steps

1. Ask the user for:
   - **Diátaxis type**: `tutorial`, `how-to`, `reference`, or `explanation`
   - **Section**: `developer`, `operator`, `users`, or `about-iota`
   - **Topic/subdirectory** (e.g. `move`, `isc`, `identity`)
   - **Title** (used to derive the slug and `# Heading`)

2. Derive the file path:
   ```
   content/<section>/<topic>/<type>s/<slug>.mdx
   ```
   Use kebab-case for the slug. Create the subdirectory if it doesn't exist.

3. Scaffold the file with the correct frontmatter and type template from CLAUDE.md.

4. Register the page in `content/sidebars/<section>.js`. Find the correct `type: 'category'` block and add the page ID inside its `items` array — do not append to the top level of the file.

5. Print the created file path and the sidebar entry added so the user can review them.

## Notes

- Check `content/_snippets/` for reusable components before writing new boilerplate.
- Tags must exist in `content/tags.yml`. Use the Diátaxis tag plus at least one technology tag.
- Never copy code inline; use `file=<rootDir>/...` (`<rootDir>` = `docs/`) or the `reference` keyword.
- Domain terms defined in `site/config/jargon.js` are auto-highlighted in prose — use them.
- If this page replaces an existing one at a different path, add a redirect in `docusaurus.config.js`.
