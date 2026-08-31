Test setup:

```
A ---> B ---> C
```

B has a Move.lock, but A and C don't.

C declares `D = { local = "../D", override = true }`. Reached through A's
manifest that edge carries the override flag; reached through B's lock file it
does not, because the lock file format does not store the flag. When the
override flag took part in dependency equality, the two views of C compared as
different and the build failed — with both sides printed identically, since the
flag is not rendered:

```
Failed to build Move modules: When resolving dependencies for package A, conflicting dependencies found:
At C
        D = { local = "../D" }
        Iota = { git = "https://github.com/iotaledger/iota.git", rev = "...", subdir = "crates/iota-framework/packages/iota-framework" }
        IotaSystem = { git = "https://github.com/iotaledger/iota.git", rev = "...", subdir = "crates/iota-framework/packages/iota-system" }
        MoveStdlib = { git = "https://github.com/iotaledger/iota.git", rev = "...", subdir = "crates/iota-framework/packages/move-stdlib" }
        Stardust = { git = "https://github.com/iotaledger/iota.git", rev = "...", subdir = "crates/iota-framework/packages/stardust" }
At B -> C
        D = { local = "../D" }
        Iota = { git = "https://github.com/iotaledger/iota.git", rev = "...", subdir = "crates/iota-framework/packages/iota-framework" }
        IotaSystem = { git = "https://github.com/iotaledger/iota.git", rev = "...", subdir = "crates/iota-framework/packages/iota-system" }
        MoveStdlib = { git = "https://github.com/iotaledger/iota.git", rev = "...", subdir = "crates/iota-framework/packages/move-stdlib" }
        Stardust = { git = "https://github.com/iotaledger/iota.git", rev = "...", subdir = "crates/iota-framework/packages/stardust" }
```
