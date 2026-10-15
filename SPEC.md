## Modes

| Mode    | Visible content      | Selectable changes | Action          |
| ------- | -------------------- | ------------------ | --------------- |
| Working | Staged + unstaged    | Unstaged           | Stage / Discard |
| Staged  | Staged only          | Staged             | Unstage         |
| Preview | Final staged content | None               | Read-only       |

## Line behavior

| Operation        | Working              | Staged               |
| ---------------- | -------------------- | -------------------- |
| Stage addition   | Becomes regular text | Appears as added     |
| Stage deletion   | Disappears           | Appears as removed   |
| Unstage addition | Appears as added     | Disappears           |
| Unstage deletion | Appears as removed   | Becomes regular text |

## Visual styling

- Regular: Default foreground, no background.
- Added: Green foreground, `+` gutter.
- Removed: Red foreground, `-` gutter.
- Cursor: Neutral full-line background

## Navigation and UI

- Persistent tabs: `Working | Staged | Preview`.
- Active tab distinguished by contrast and underline.
- Context-sensitive footer: `Stage selection`, `Unstage selection`, or `Read only`.
- navigate tabs with `[` and `]`
  - switching tabs disabled during multi-line selection mode, other tabs should appear greyed out

## Navigation modes

- tab to toggle between line/hunk selection modes
- tab disabled if there is an unsubmitted line selection
- switching to Hunk mode focuses the hunk containing the current line.

## Interaction

- Confirmation on discard unstaged changes.

## Line mode

- modes: single (default), multi
- single-line mode
  - Cursor highlights one line.
  - `j/k`: navigate
  - `Space`: stage/unstage line
  - `v`: switch to multi-line mode
- multi-line mode
  - Cursor highlights current line.
  - selected lines slightly highlighted
  - `j/k`: Extend or shrink the selection.
  - `Space`: stage/unstage selection
  - `Esc`: return to single-line mode

## Hunk mode

- `j/k`: Navigate hunks
- `Space`: stage/unstage hunk.
- Cursor highlights the entire hunk, without a separate line cursor.

