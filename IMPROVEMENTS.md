# pick-ssh — improvement ideas

Roughly ordered by ROI. Bugs and code-quality fixes at the bottom are small and contained.

## High-value features

1. **Reachability indicator.** Cache the last `Ctrl+T` test result (or run a fast TCP-port-22 probe in the background for visible hosts) and render a coloured dot in the prefix column. Persist across sessions in the YAML config.

2. **Edit-in-place.** A keybind to open `$EDITOR` on the `~/.ssh/config` (or the right `Include`'d file) at the line of the selected entry. The parser already knows the source path per node.

3. **Add-new-host form.** A small popup capturing alias / HostName / User / Port / IdentityFile, then appended to the chosen file. Closes the loop for a tool you reach for daily.

4. **Alternative actions.** Beyond ssh/scp: `mosh`, `ssh-copy-id`, `sshfs`, `tmux new -s host`, `code --remote ssh-remote+host`. Could be a sub-popup ("Action: ssh / mosh / scp / sshfs…") triggered with e.g. `Ctrl+A`.

## Bugs / code quality

6. **`Host foo bar` aliases collapse.** `ssh_config.rs:58-66` stores `value` (the whole string) as `name`. SSH treats `Host foo bar baz` as three aliases sharing one block — currently they all show up as a single entry called "foo bar baz". Split on whitespace when no glob/`!` is present and emit one `HostEntry` per alias.

7. **`app.rs` is 1043 lines.** Split into focused submodules: `app/filter.rs`, `app/sftp.rs`, `app/ssh_test.rs`, `app/pinned.rs`. Same for `event.rs` (one big match handling 5 popup contexts).

8. **SFTP via `ssh host ls`.** `app.rs:783-794` opens a fresh ssh connection per directory listing. Enable `ControlMaster=auto ControlPersist=60s` on the test/sftp invocations — directory navigation becomes near-instant on slow links.

9. **Mouse wheel step.** Main list moves 1 row per scroll click (`event.rs:244-245`); the ssh-test popup uses 3 (`event.rs:235-238`). Make main list scroll 3 too.

## Polish

10. **Status footer.** "12 config • 47 history • 3 pinned" in a thin bottom bar — quick orientation.
11. **Persistent last filter** (optional, `--remember-filter`).
12. **Tags/groups.** Free-form labels saved per host in the YAML; filter syntax `@prod` to search by tag.
13. **`-V/--version`** flag and a richer `--help` (the current one barely lists flags).
