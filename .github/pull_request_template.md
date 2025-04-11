# Description of change

Please write a summary of your changes and why you made them.

## Links to any relevant issues

Be sure to reference any related issues by adding `fixes #(issue)`.

## Type of change

Choose a type of change, and delete any options that are not relevant.

- Bug fix (a non-breaking change which fixes an issue)
- Enhancement (a non-breaking change which adds functionality)
- Breaking change (fix or feature that would cause existing functionality to not work as expected)
- Documentation Fix

## How the change has been tested

Describe the tests that you ran to verify your changes.

Make sure to provide instructions for the maintainer as well as any relevant configurations.

## Change checklist

Tick the boxes that are relevant to your changes, and delete any items that are not.

- [ ] I have performed Basic tests (linting, compilation, formatting, unit/integration tests)
- [ ] I have performed Patch-specific tests (correctness, functionality coverage)
- [ ] I have followed the contribution guidelines for this project
- [ ] I have performed a self-review of my own code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have made corresponding changes to the documentation
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] I have checked that new and existing unit tests pass locally with my changes

### Infrastructure QA

Tick the boxes that are relevant to your changes. For any unchecked item, authors must provide a justification that describes why the test was not necessary.

- [ ] I tested the synchronization of the indexer from genesis on a network that includes the migration objects.
- [ ] I tested the synchronization of the indexer locally without resetting the database.
- [ ] I tested the synchronization of the indexer on a production-like database.
- [ ] I tested the deployment of the services with docker.
- [ ] I ensured backward compatibility of the APIs.
