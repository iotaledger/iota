# Troubleshooting

## IOTA Framework change

If IOTA framework code got updated, the expectations need to be changed. Follow these steps:

```bash
# required; can be omitted if cargo-insta is installed
$ cargo install cargo-insta

# run in ./iota-cost
$ cargo insta test --review
```
