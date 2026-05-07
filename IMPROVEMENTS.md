# pick-ssh — improvement ideas

Roughly ordered by ROI. Bugs and code-quality fixes at the bottom are small and contained.

## High-value features

1. **Filter matches `info` too.** `app.rs:309-313` only fuzzy-matches `label` and `full_cmd`. Hosts whose aliases are opaque (`main-a1`) but whose `HostName`/IP/jump host you remember are unsearchable. Trivial fix in `update_filter`.

2. **Ranked fuzzy matching.** Current `fuzzy_match` is boolean. With many hosts, results stay in original order. Add an fzf-style score (consecutive-char bonus, word-boundary bonus, prefix bonus) and sort matches; preserve section order when no filter, sort within section when there is one. Or pull in the `fuzzy-matcher` / `nucleo` crate for free.

3. **`~/.ssh/known_hosts` as a source.** Many hosts you've connected to don't have a `Host` block. Parse known_hosts as another section (filter out hashed entries).

4. **Reachability indicator.** Cache the last `Ctrl+T` test result (or run a fast TCP-port-22 probe in the background for visible hosts) and render a coloured dot in the prefix column. Persist across sessions in the YAML config.

5. **Edit-in-place.** A keybind to open `$EDITOR` on the `~/.ssh/config` (or the right `Include`'d file) at the line of the selected entry. The parser already knows the source path per node.

6. **Add-new-host form.** A small popup capturing alias / HostName / User / Port / IdentityFile, then appended to the chosen file. Closes the loop for a tool you reach for daily.

7. **Alternative actions.** Beyond ssh/scp: `mosh`, `ssh-copy-id`, `sshfs`, `tmux new -s host`, `code --remote ssh-remote+host`. Could be a sub-popup ("Action: ssh / mosh / scp / sshfs…") triggered with e.g. `Ctrl+A`.

## Bugs / code quality

8. **`Host foo bar` aliases collapse.** `ssh_config.rs:58-66` stores `value` (the whole string) as `name`. SSH treats `Host foo bar baz` as three aliases sharing one block — currently they all show up as a single entry called "foo bar baz". Split on whitespace when no glob/`!` is present and emit one `HostEntry` per alias.

9. **`app.rs` is 1043 lines.** Split into focused submodules: `app/filter.rs`, `app/sftp.rs`, `app/ssh_test.rs`, `app/pinned.rs`. Same for `event.rs` (one big match handling 5 popup contexts).

10. **`fuzzy_match` control flow.** `app.rs:87-104` has an early-return into a sibling helper (`fuzzy_match_lower`) when case-insensitive — dead code in the case-sensitive branch's loop. Either inline the lowercase normalization or always go through the char-vector path.

11. **SFTP via `ssh host ls`.** `app.rs:783-794` opens a fresh ssh connection per directory listing. Enable `ControlMaster=auto ControlPersist=60s` on the test/sftp invocations — directory navigation becomes near-instant on slow links.

12. **Mouse wheel step.** Main list moves 1 row per scroll click (`event.rs:244-245`); the ssh-test popup uses 3 (`event.rs:235-238`). Make main list scroll 3 too.

## Polish

13. **Status footer.** "12 config • 47 history • 3 pinned" in a thin bottom bar — quick orientation.
14. **Persistent last filter** (optional, `--remember-filter`).
15. **Tags/groups.** Free-form labels saved per host in the YAML; filter syntax `@prod` to search by tag.
16. **`-V/--version`** flag and a richer `--help` (the current one barely lists flags).
