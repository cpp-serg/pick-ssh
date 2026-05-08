# pick-ssh — improvement ideas

Roughly ordered by ROI. Bugs and code-quality fixes at the bottom are small and contained.

## High-value features

1. **Reachability indicator.** Cache the last `Ctrl+T` test result (or run a fast TCP-port-22 probe in the background for visible hosts) and render a coloured dot in the prefix column. Persist across sessions in the YAML config.

2. **Alternative actions.** Beyond ssh/scp: `mosh`, `ssh-copy-id`, `sshfs`, `tmux new -s host`, `code --remote ssh-remote+host`. Could be a sub-popup ("Action: ssh / mosh / scp / sshfs…") triggered with e.g. `Ctrl+A`.

## Bugs / code quality

3. **`Host foo bar` aliases collapse.** `ssh_config.rs:58-66` stores `value` (the whole string) as `name`. SSH treats `Host foo bar baz` as three aliases sharing one block — currently they all show up as a single entry called "foo bar baz". Split on whitespace when no glob/`!` is present and emit one `HostEntry` per alias.

4. **`app.rs` is now ~1900 lines.** Split into focused submodules: `app/filter.rs`, `app/sftp.rs`, `app/ssh_test.rs`, `app/pinned.rs`, `app/add_host.rs`. Same for `event.rs` (big match handling many popup contexts).

5. **SFTP via `ssh host ls`.** `app.rs` opens a fresh ssh connection per directory listing. Enable `ControlMaster=auto ControlPersist=60s` on the test/sftp invocations — directory navigation becomes near-instant on slow links.

6. **Mouse wheel step.** Main list moves 1 row per scroll click; the ssh-test popup uses 3. Make main list scroll 3 too.

## Polish

7. **Status footer.** "12 config • 47 history • 3 pinned" in a thin bottom bar — quick orientation.
8. **Persistent last filter** (optional, `--remember-filter`).
9. **Tags/groups.** Free-form labels saved per host in the YAML; filter syntax `@prod` to search by tag.
10. **`-V/--version`** flag and a richer `--help` (the current one barely lists flags).
