# git-difftui

A one-file Git staging terminal UI for Linux and macOS, built with Rust and Ratatui.
Requires a recent Git CLI (2.31+) and a Rust toolchain compatible with Ratatui 0.29.

Build with `cargo build --release`, then run `target/release/git-difftui <file>`
from your repository context. Install with `cargo install --path .`.

## Controls

- `[` / `]`: Working / Staged / Preview
- `j` / `k` (or arrows): next/previous change; scroll in Preview
- `Tab`: line/hunk mode (hunk mode by default)
- `v`: anchored multi-line selection; cannot cross unchanged text
- `Ctrl-U` / `Ctrl-D`: scroll half a page without moving the cursor
- `Esc`: cancel selection, or center the cursor when not selecting
- `Space`: stage (Working) or unstage (Staged)
- `d`: request discard in Working; only `y` confirms, `n`/`Esc` cancels
- Left/right arrows: horizontal scroll
- `q` or Ctrl-C: quit

Changes apply immediately. There is no commit or save step. Working compares the
index with disk, Staged compares HEAD with the index, and Preview shows the index.
Replacement deletions and additions are independently selectable. Submission focuses
the next remaining change below, falling back upward; a message indicates completion.
Hunk highlighting includes only changed rows, not context.

## Safety and scope

Supports partial staging and unstaging of existing ordinary UTF-8 text files
already in Git's index, including newly added files absent from HEAD and repositories
without a first commit. Indexed additions use an empty HEAD baseline; unstaging all
their content preserves an empty index entry. The app does not add untracked files
or provide whole-file management operations. Binary/NUL-containing text, symlinks,
submodules, conflicts, deletions, and mode/type changes are refused. Non-UTF-8 paths
are unsupported.
Git clean filters, encoding/ident/text/EOL attributes and implicit EOL conversion
are conservatively refused rather than silently transforming content. Byte-exact
LF and CRLF text, including missing final newlines, are supported. Rows retain
their exact original bytes, so independently combining non-newline-terminated
rows does not invent a separating newline.

The repository is discovered from the invocation directory, not the file's
location. Git environment overrides are respected. A snapshot is refreshed every
800 ms, except while selecting multiple lines or confirming discard. Every action
is revalidated against current HEAD/index/file content before writing; a stale
action is cancelled, never retargeted by row number. Unsupported external states
disable mutations until a successful refresh.

Index updates hold Git's real `index.lock`, copy the current index, update only
the target content/mode entry, and atomically publish the result. Other entries
are preserved. Discard prepares a same-directory temporary file, preserves owner,
group and permission bits, revalidates, and atomically replaces the content.
Extended attributes (including ACLs) are copied; an attribute-copy failure aborts
discard. Failure before publication leaves the original intact. As with other
filesystem editors, non-cooperating processes can still race the final
validation/rename; Git writers must honor Git locking. Discard replaces the file
inode; hard-linked aliases are not edited. Filesystem-specific flags are not
supported.

Run automated diff, navigation and real-repository tests with `cargo test`.
