# Agent memory filesystem

Persistent notes for Cursor agents working on WiiMaker. **Not** a substitute for
`README.md` / `ARCHITECTURE.md` / `.cursor/rules` — those are the canonical docs.
`MEMORY/` holds learned facts, session scratch, and cross-chat continuity.

## Layout

```
MEMORY/
├── README.md          # this contract
├── durable/           # committed — long-lived agent knowledge
│   ├── index.md       # table of contents + pointers
│   ├── engine.md      # core / host / assets / runtime / games
│   ├── editor.md      # egui editor + scene mutate UX notes
│   └── cli.md         # CLI / agent workflows / --json quirks
└── sessions/          # gitignored — ephemeral per-chat working notes
    └── YYYY-MM-DD-<topic>.md
```

## When to read

At the start of non-trivial work (and whenever `@MEMORY` is mentioned):

1. Read `MEMORY/durable/index.md`
2. Read the durable file(s) for the area you are touching (`engine` / `editor` / `cli`)
3. If a matching file exists under `MEMORY/sessions/`, skim it for in-flight context

## When to write

| Destination | Write when |
|---|---|
| `durable/*.md` | Stable gotchas, API quirks, decisions, “do this next time”, broken assumptions you corrected |
| `sessions/*.md` | Scratch for the current task: plan, blockers, commands run, open questions |

Keep durable notes **short bullets**. Prefer linking to source paths (`crates/...`) over pasting large code.

## Rules

- Never store secrets, tokens, or private keys in `MEMORY/`.
- Do not duplicate architecture — point at `ARCHITECTURE.md` / rules instead.
- Update durable memory when you learn something future agents will need; prune stale bullets.
- Session files are disposable; promote anything lasting into `durable/` before finishing.
- One concern per durable file; use `index.md` to cross-link.
