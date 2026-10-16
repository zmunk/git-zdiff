# git-difftui

## Purpose and authority

Build a working, single-file Git staging terminal application named `git-difftui`.
This document is the standalone, authoritative specification. It incorporates the
original UI requirements and all subsequent design decisions; no design-tree file
or conversation history is required to implement the application.

The tool operates on partial **content changes**, not file-level Git operations.

## Technology and platforms

- Implement in Rust using **Ratatui** for the terminal UI.
- Use the installed **Git CLI** as the Git backend, not an embedded Git library.
- Standard Cargo dependencies are allowed.
- Support Linux and macOS. Windows support is not required.

## Invocation and repository context

```text
git-difftui <file>
```

- Open exactly one file per launch. There is no file browser, sidebar, or multi-file view.
- Resolve relative file paths from the invocation's current working directory.
- Use the same repository context/discovery behavior as `git diff` invoked from
  that directory. Do not discover a different repository by walking from the
  target file or changing into its directory.
- Start in the Working tab, in single-line selection mode.
- `q` quits without performing further Git or working-tree mutations. Completed
  operations are already applied; quitting neither rolls them back nor commits them.
- Invalid arguments, an unavailable repository, and unsupported inputs must produce
  clear errors without modifying repository or file content.

## Supported inputs and scope

Support partial content staging and unstaging of **existing, ordinary text files
already present in Git's index**: both files present in HEAD and newly added files
absent from HEAD. Repositories without a first commit are supported. The file must
exist on disk and must not be a staged deletion. Do not add untracked files or
provide whole-file management operations.

Reject or clearly report as unsupported:

- Untracked files. Tell the user to track the file before opening it; do not track it automatically.
- Deleted files, whether the deletion is staged or unstaged.
- Renames and other file-level operations.
- Binary files.
- Merge-conflicted files.
- Symlinks and submodules.
- Permission-only changes; file-mode operations are outside scope.

Do not create, delete, rename, or change the type or permissions of files as an
application action. Do not stage file-level metadata changes as a side effect of
a content operation.

Content operations **may leave a file empty**. Preserve both its filesystem path
and its index entry: empty content is allowed, but file deletion is not. Unstaging
all content of an indexed addition leaves an empty indexed file, not an untracked
file. Its existing index mode is preserved; no new file-level metadata is staged.

## Git content model

Use three versions of the target file:

- **HEAD:** committed content, or an empty content baseline for an indexed addition
  absent from HEAD (including before the first commit).
- **Index:** staged content.
- **Working tree:** content on disk.

| Tab | Display | Selectable changes | Action |
| --- | --- | --- | --- |
| Working | Full inline index-to-working-tree diff | Unstaged additions/deletions | Stage or discard |
| Staged | Full inline HEAD-to-index diff | Staged additions/deletions | Unstage |
| Preview | Full index content | None | Read-only |

### Meaning of the Working tab

The index is the baseline. Staged content is ordinary baseline text, not a second
set of marked changes. Only differences between the index and working tree are
marked as changes.

Consequently, staged additions become regular text in Working, and staged
deletions disappear from Working. Do not combine HEAD-to-index and
index-to-working-tree diffs into a single HEAD-to-working-tree diff: that can hide
staged changes reversed by subsequent unstaged edits.

### Full-file rendering and hunks

- Working and Staged display the **entire file** with inline diff rows, including
  unchanged content outside hunks. Do not show only context-limited excerpts.
- Added rows represent content in the newer version; removed rows represent
  content in the baseline. Both sides of a replacement appear as separate rows.
- Group changes into hunks using standard Git-style **three-context-line** hunk
  grouping. Context rows may belong to a hunk but are never actionable.
- Distinguish hunk grouping from selection boundaries: multi-line selection cannot
  cross unchanged rows even when both changes belong to the same hunk.
- Preview displays the entire index file without diff additions or removals.

## Content operations

| Operation | Working result | Staged result |
| --- | --- | --- |
| Stage addition | Becomes regular text | Appears as added |
| Stage deletion | Disappears | Appears as removed |
| Unstage addition | Appears as added | Disappears |
| Unstage deletion | Appears as removed | Becomes regular text |

These descriptions assume no overlapping changes that alter the resulting diff;
all views must ultimately be derived from the actual updated Git content.

- **Stage:** apply only the selected unstaged content change(s) to the index.
  Do not modify the working-tree file.
- **Unstage:** reverse only the selected staged content change(s) in the index,
  toward HEAD. Do not modify the working-tree file.
- **Discard:** reverse only the selected unstaged content change(s) in the working
  tree, toward the index. Do not modify the index or staged changes.
- Preserve unrelated content changes, including changes elsewhere in the same file,
  and preserve all other files and index entries.
- Operations take effect immediately after successful execution; there is no
  additional save/submit phase.

### Independent replacement lines

A replacement such as `-old` followed by `+new` is **not** an indivisible pair.
Each changed row can be staged, unstaged, or discarded independently where the
current tab permits that operation.

For example, starting with `old` in the index and `new` in the working tree:

- Staging only `+new` leaves both `old` and `new` in the index, in diff order.
- Staging only `-old` removes `old` from the index without adding `new`.
- Staging both produces `new` in the index.

The corresponding inverse semantics apply to unstaging and discarding.

## Visual styling and layout

- Persistent tabs: `Working | Staged | Preview`.
- Distinguish the active tab using contrast and underline.
- Regular text: default foreground, no background.
- Added text: green foreground with a `+` gutter marker.
- Removed text: red foreground with a `-` gutter marker.
- Cursor: neutral full-line background.
- Multi-line selected rows: subtle background highlighting, distinct from the
  current cursor row.
- Hunk-mode cursor: highlight only actionable rows in the hunk, without a separate line cursor.
- During multi-line selection, grey out the other tabs.
- Context-sensitive footer:
  - Working: `Stage selection`, with discard availability discoverable.
  - Staged: `Unstage selection`.
  - Preview: `Read only`.
- Surface confirmation prompts and operation/refresh errors visibly.

## Navigation and selection

### Common controls

| Key | Behavior |
| --- | --- |
| `[` / `]` | Switch to the previous/next tab |
| `Tab` | Toggle line/hunk selection mode in Working and Staged |
| `j` / `k` | Move down/up according to the active navigation mode |
| `Space` | Stage in Working; unstage in Staged |
| `v` | Enter multi-line selection from single-line mode |
| `Esc` | Cancel multi-line selection; otherwise center the cursor vertically |
| `Ctrl-U` / `Ctrl-D` | Scroll up/down half a viewport without moving the cursor |
| `d` | Request discard in Working |
| `q` | Quit without further mutations |

- Tab switching is disabled during multi-line selection.
- `Tab` is disabled while an unsubmitted multi-line selection exists.
- Working and Staged remember their own cursor position and line/hunk mode when
  switching tabs.
- Switching into hunk mode focuses the hunk containing the current line.
- In diff tabs, a line cursor may **only** rest on changed, actionable rows.
  Unchanged rows are visible but cannot receive the line cursor.
- When no actionable changes exist, show the available content/empty state without
  a change cursor; mutation keys do nothing.

### Line mode: single

- Highlight exactly one changed row as the cursor.
- `j`/`k` moves between changed rows, skipping unchanged text.
- `Space` stages/unstages the current row.
- `v` enters multi-line mode and immediately selects the current row as its anchor.

### Line mode: multi

- Maintain a fixed selection anchor and a moving cursor endpoint.
- `j`/`k` extends or shrinks the contiguous selection around the anchor.
- The cursor must not move onto, past, or over an unchanged row. A multi-line
  selection cannot bridge separate changed runs separated by context.
- Highlight selected rows subtly and the current cursor row distinctly.
- `Space` stages/unstages the selected rows.
- On successful submission, return to single-line mode.
- `Esc` cancels the selection without modifying any content.

### Hunk mode (default)

- `j`/`k` moves between hunks.
- Highlight only actionable rows in the current hunk, without a separate line cursor.
- `Space` stages/unstages all changed rows in the current hunk.
- Context in the hunk is not itself acted on.
- `d` in Working requests discard of all unstaged changes in the current hunk.

### Preview

- Show full index content, read-only.
- `j`/`k` scrolls the content; there is no selection cursor.
- Disable line/hunk switching, multi-line selection, and mutation controls.
- If index content is unavailable, show an explanatory empty state rather than
  inventing content. This does not expand the supported-input scope.

### Cursor retention

After a successful submission, focus the first remaining actionable row below the
submitted range, or the previous actionable row above if none remain below. If no
changes remain, remove the cursor and display a completion message. After an external
refresh, preserve focus on the same logical change wherever possible, falling back
to the nearest actionable change. Do not blindly retain a row number when rows shift.
Manual half-page scrolling must persist across redraws without moving the cursor.
Cursor navigation ensures visibility; Escape outside multi-line selection centers
the cursor (as near the top of the file as available content allows).

## Discard confirmation

Discard is available **only in Working** and always requires confirmation.

- `d` requests discard of the current line, multi-line selection, or hunk.
- Explain the target/scope in the confirmation prompt.
- `y` confirms; `n` or `Esc` cancels.
- Default to cancellation; confirmation must be explicit.
- Cancellation leaves the file and index untouched.
- Confirmation does not waive external-change validation: revalidate before writing.

## External changes, refreshing, and mutation safety

Other programs may edit the working file or change Git state while the app runs.

- Automatically refresh when **not** in multi-line selection mode.
- During multi-line selection, do **not** automatically replace the displayed
  snapshot or selection. Defer refresh until an operation is initiated.
- Validate every mutation against current Git state before applying it, including
  after a discard confirmation prompt.
- If an external change makes the requested mutation or selection invalid, do not
  apply it. Cancel the invalid selection/action, refresh, and report what happened.
- Never apply a stale selection to different content merely because the row numbers
  or patch positions still appear usable.
- Preserve focus on the same logical change where possible after refresh.
- External changes unrelated to the target must not be overwritten by an index
  update. Respect Git locking and report lock/write failures without claiming success.
- Failed operations must not silently apply a different scope or fall back to
  staging/unstaging/discarding the entire file.
- If external changes make the input unsupported, report that condition and prevent
  further mutations while it remains unsupported.

The refresh mechanism and timing are implementation choices. Correctness does not
depend on detecting every change through a watcher: mutation-time validation is
required regardless of the refresh mechanism.

## Implementation latitude

The decisions above are requirements. Internal module boundaries, dependency
versions, terminal backend, refresh interval/mechanism, exact error wording,
nonessential layout details, and extra nonconflicting convenience controls are
implementation choices. They must not change the specified selection, content,
repository-discovery, or safety semantics.

Implement robust terminal cleanup on normal exit and errors. Preserve content
outside the selected operation, including line endings and final-newline state;
unsupported text representations must fail clearly rather than be silently
rewritten. Git failures must be reported, not mistaken for empty diffs.

## Acceptance criteria

Use automated tests where practical, including temporary real Git repositories
for content operations. The application is complete when the following behaviors
are demonstrated:

1. Build and run the Ratatui application as `git-difftui <file>` on the supported
   platforms, with one-file scope and Git-equivalent repository discovery.
2. Show all three tabs with the specified full-file models, styling, footer, and
   read-only Preview behavior.
3. Verify each row of the stage/unstage behavior table, including additions,
   deletions, overlapping staged/unstaged edits, and independent replacement rows.
4. Stage/unstage one line, a bounded multi-line selection, and a hunk without
   altering unrelated content or the working tree.
5. Confirm single-line navigation skips context, multi-line selection cannot cross
   it, tab/mode switches are blocked during selection, and successful selection
   submission returns to single-line mode.
6. Verify tab-local navigation state, hunk focus on mode switching, logical cursor
   retention, and the no-actionable-changes state.
7. Discard only the requested unstaged content after explicit confirmation; cancel
   safely and preserve staged content.
8. Permit empty content without removing a filesystem path or index entry.
   Cover partial staging/unstaging of indexed additions, including before the first
   commit, and unstaging all added content while retaining an empty index entry.
9. Reject unsupported input types without mutation; untracked-file errors ask the
   user to track the file first.
10. Refresh automatically outside selection, defer refresh during selection, and
    reject invalidated selections/mutations without applying stale changes.
    Cover external changes during discard confirmation and changes to other index
    entries.
11. Handle Git/write/lock failures visibly without destructive fallback, and
    restore the terminal on exit or error.
