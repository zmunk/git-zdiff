# Performance Analysis: git-difftui

## Problem

Each stage/unstage/discard operation has a noticeable delay. The UI feels sluggish
compared to tools like lazygit.

## Root cause: per-keystroke subprocess storm

Every mutation triggers 2–3 full `snapshot()` calls (pre-validation, sometimes
mid-operation, and post-mutation refresh). Each snapshot spawns ~7 separate Git
processes. With the mutation's own 1–2 Git processes, that's **~16 Git subprocess
spawns per keystroke**.

```
snapshot() does:
  git ls-files --stage -z -- <spec>
  git ls-tree --full-tree -z HEAD -- <spec>
  git check-attr -z filter working-tree-encoding ident text eol -- <path>
  git cat-file blob <head_oid>
  git cat-file blob <index_oid>
  read working file from disk
  git hash-object --no-filters --stdin
  git hash-object --path=<path> --stdin
```

Process creation overhead (fork+exec) is the bottleneck, especially on macOS.

## How lazygit achieves snappiness

| Technique | Description |
|---|---|
| Persistent batch processes | Opens one long-lived `git cat-file --batch` process, pipes OID requests to it. Zero fork/exec per blob read. |
| `git diff` for diffs | Uses `git diff --cached` and `git diff` directly instead of reading blobs and diffing in-process. |
| `git apply --cached` for staging | Pipes selected patch hunks to `git apply --cached` rather than reconstructing file content, hashing, and calling `update-index`. |
| Cached model | Builds an internal repo model and updates it incrementally. Only re-reads from Git when an external change is detected (via file watcher or `git status` polling). |
| Targeted invalidation | After its own mutations, lazygit updates the model directly — it knows what changed. No need to re-snapshot. |

Net result: ~1–2 Git invocations per action instead of ~16.

## Optimization plan

### 1. Persistent `git cat-file --batch` process

Maintain a long-lived child process:

```
git cat-file --batch
```

Write `<oid>\n` to stdin, read back `<oid> <type> <size>\n<content>`.

**Saves:** 2 process spawns per snapshot (HEAD blob + index blob reads).

### 2. Use `git diff` for diff computation

Replace in-process `similar::TextDiff` on the three blobs with:

- `git diff --cached` → Staged tab (HEAD vs index)
- `git diff` → Working tab (index vs worktree)

Parse the unified diff output into the internal `Diff` representation. Git
computes the diff internally; no blobs need to be read at all for display.

**Saves:** 3 blob reads + in-process diff computation. Replaced by 2 git
invocations total for both tabs.

### 3. Use `git apply --cached` for stage/unstage

Instead of:

1. Reconstruct full file content from selected diff rows
2. `git hash-object -w --stdin` to write a new blob
3. `git update-index -z --index-info` to update the index entry

Do this instead:

- Generate a patch from the selected diff rows
- Pipe it to `git apply --cached` (to stage)
- Pipe it to `git apply --reverse --cached` (to unstage)

This is what `git add -p` and lazygit use internally.

**Saves:** 2 git processes per mutation, plus eliminates manual content
reconstruction.

### 4. Cache the snapshot; don't re-read on every keystroke

- Keep `Snapshot` as cached state inside `Repo`.
- After your own mutation, update the model directly — you know exactly what
  rows were staged/unstaged/discarded and what the new content is.
- Use a file watcher (`inotify` on Linux, `kqueue` on macOS) or poll
  `git status --porcelain` to detect external changes.
- Only run a full re-snapshot when something external actually changed.
- Eliminates pre-validation and post-mutation snapshot calls entirely. The
  snapshot carried in the `Request` is already validated at mutation time.

**Saves:** 2 full snapshots (~14 git processes) per mutation.

### 5. Run expensive checks once at startup, not per-refresh

Move these from `snapshot()` to `open()`:

- `git check-attr` — text transformation attributes don't change at runtime.
- Double-hash of working file (with/without filters) — same reasoning.
- `git ls-tree HEAD` — HEAD doesn't change during a session (no commits are
  made inside the tool).

**Saves:** 3 git processes per snapshot.

### 6. (Optional) Direct index manipulation via `git2` crate

Use libgit2 bindings to read and write the index directly, bypassing the Git
CLI for index operations. This eliminates all remaining subprocess overhead for
mutations. However, it adds a large native dependency and increases complexity.

## Expected outcome

| Metric | Current | After |
|---|---|---|
| Git processes per stage/unstage | ~16 | **1** (`git apply --cached`) |
| Git processes per refresh | ~7 | **0–2** (only if external change detected) |
| Blob reads per action | 2–4 | **0** |
| Subprocess spawns per keystroke | 16 | 1 |
| 800ms polling loop cost | Always runs snapshot | Usually a no-op (watcher says clean) |

Each keystroke becomes a single lightweight git subprocess call. File watching
makes the periodic refresh essentially free when nothing changed externally.
This matches lazygit's performance profile.
