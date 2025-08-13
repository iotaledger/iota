If you need to update the fixtures in `tests/fixtures`, follow these steps:

1. Update the `SELECTED_EPOCHS` constant in `src/bin/update_fixtures.rs` with the epoch(s) you intend to use in your test(s). Do not remove already existing entries or otherwise tests might break.
2. Run `cargo run --bin update-fixtures`. This will download the corresponding end-of-epoch checkpoints and checkpoint summaries. However, it will not automatically remove no longer used fixtures.
