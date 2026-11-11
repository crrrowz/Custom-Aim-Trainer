# Task prompt template

The reviewer fills this in for one backlog item and gives it to the executor. Keep every section; write
"none" rather than deleting one. A prompt is ready only when sections 3, 5 and 6 are concrete.

```markdown
# Task <ID>: <one-line goal>

## 0. Rules
Read `AGENTS.md` in full first and follow it: Trail/Graft before reading source, a branch from fresh
`main`, test first (the test must fail before the fix; keep the failure lines), one change per commit,
all gates green before each commit, no push to `main`, no force/rebase/amend. Reply to the user in Iraqi
Arabic; code, commits and docs in English.

## 1. Context
<What is wrong or missing, for whom, and why it matters. Point to evidence: file:line, a screenshot,
a log line. 3-6 lines.>

## 2. Branch
`<fix|feat|chore|docs>/<ID>-<slug>` from fresh `main` (worktree: `../Custom-Aim-Trainer-wt/<slug>`).

## 3. Allowed files
- `<path>`: <what may change there>
Anything else: stop and report why it is needed.

## 4. Behaviour
<Exactly what must be true afterwards, including edge cases and what must NOT change.>

## 5. Tests (write these first)
- `<test_name>` in `<path>`: <what it checks; why it fails today>
- ...

## 6. UI / UX (when anything visible changes)
- Layout and hierarchy: <...>
- States: empty / busy / error / normal: <...>
- Text in both languages: <ar> / <en>
- Visual check: <page and action>; attach screenshots (both RTL and LTR when the layout changes).
UI changes get their own commit.

## 7. Acceptance
- [ ] All gates in `AGENTS.md` section 3 green on this branch.
- [ ] <item-specific checks>
- [ ] `docs/BACKLOG.md`: <ID> moved to Done (PR number filled in after the PR exists).
- [ ] PR opened to `main` with the report as its description: <yes/no>

## 8. Report
Use `AGENTS.md` section 7. Say plainly what you could not verify.
```

## Notes for the reviewer

- One backlog item per prompt. Split anything that touches more than one area (backend, UI, trainer).
- Name the tests: a vague "add tests" produces tests that cannot fail.
- Never forward work the user refused, and never ask the executor to touch the user's real data.
- After the report: read the diff (only allowed files?), read the new tests (can they fail?), re-run
  the gates yourself, then merge or send back precise corrections.
