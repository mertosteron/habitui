---
title: Agent Teams — Master Reference
purpose: Authoritative working reference for designing, spawning, and operating Claude Code agent teams.
source: https://docs.claude.com/en/docs/claude-code/agent-teams (and related pages)
status: experimental — requires CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1 and Claude Code v2.1.32+
---

# Agent Teams — Master Reference

A focused, practical guide for building effective agent teams. Use this as a checklist before spawning a team, a debugger when one misbehaves, and a design reference when shaping new workflows.

---

## 1. What an agent team is

A coordinated set of independent Claude Code sessions:

- **Lead** — the session that creates the team, spawns teammates, assigns tasks, synthesizes results. Fixed for the team's lifetime; cannot be transferred.
- **Teammates** — separate Claude Code instances, each with its own context window, permissions inherited from the lead, and access to project context (CLAUDE.md, MCP servers, skills).
- **Shared task list** — work items teammates claim and complete; supports dependencies and file-locked claiming.
- **Mailbox** — direct messaging by name between any agents (lead↔teammate, teammate↔teammate).

Storage:
- Team config: `~/.claude/teams/{team-name}/config.json` (auto-managed; never hand-edit)
- Task list: `~/.claude/tasks/{team-name}/`

There is **no project-level team config**. Files like `.claude/teams/teams.json` are not recognized.

---

## 2. When to use a team (vs. alternatives)

### Use a team when
- Multiple **independent angles** on a problem benefit from parallel exploration.
- Workers need to **talk to each other**, challenge findings, and converge.
- Work spans **disjoint files/modules** with little sequential dependency.
- Debugging benefits from **competing hypotheses**.
- Cross-layer work (frontend + backend + tests) where each layer has a clear owner.

### Prefer subagents when
- You only need a result returned to the main agent — no inter-worker discussion.
- The task is focused and bounded, and token efficiency matters.
- You don't need to interact with the worker directly.

### Prefer a single session when
- The work is sequential or has many dependencies.
- Edits concentrate on the same files.
- The task is routine (refactors, bug fixes with tests, doc updates).

### Quick decision matrix

| Need | Use |
|---|---|
| One-off result, no discussion | Subagent |
| Parallel + cross-talk + steerable | Agent team |
| Same-file or sequential edits | Single session |
| Manual control of N sessions | Git worktrees |

---

## 3. Prerequisites and setup

1. **Version**: `claude --version` ≥ 2.1.32.
2. **Enable** the experimental flag:
   ```json
   // settings.json
   { "env": { "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1" } }
   ```
3. **Display mode** (optional): `teammateMode` in `~/.claude/settings.json`:
   - `"in-process"` — all teammates in main terminal; cycle with Shift+Down. Works anywhere.
   - `"tmux"` — split panes; auto-detects tmux or iTerm2 (`it2` CLI + Python API enabled).
   - `"auto"` (default) — split if already in tmux, otherwise in-process.
4. Override per session: `claude --teammate-mode in-process`.

> Split-pane mode is **not** supported in VS Code's integrated terminal, Windows Terminal, or Ghostty.

---

## 4. Designing an effective team

### Principles
- **Independent angles only.** Two teammates working on the same file cause overwrites.
- **Distinct lenses.** Give each teammate a different perspective, role, or hypothesis — not just a different task.
- **Clear deliverable per teammate.** A self-contained unit (a function, a file, a written report).
- **Bounded scope per task.** Aim for ~5–6 tasks per teammate; smaller is better than larger.

### Sizing the team
- Start with **3–5 teammates** for most workflows.
- Add more only if the work genuinely parallelizes.
- Three focused teammates beat five scattered ones.
- Coordination overhead and token cost scale with team size — both grow faster than throughput.

### Sizing tasks
- **Too small** → coordination overhead dominates.
- **Too large** → teammate runs too long without check-ins; risk of wasted effort.
- **Just right** → discrete, verifiable deliverable.

### Naming teammates
- The lead assigns names at spawn. **Specify desired names in your prompt** if you want predictable handles for later messages (e.g. `name them researcher, architect, devil-advocate`).

---

## 5. Spawning a team — prompt patterns

The lead spawns teammates from your natural-language prompt. Good prompts specify: roles, count, model (optional), names (optional), and per-teammate context.

### Pattern A — Parallel review (independent lenses)
```
Create an agent team to review PR #142. Spawn three reviewers:
- One focused on security implications
- One checking performance impact
- One validating test coverage
Have them each review and report findings.
```

### Pattern B — Adversarial debate (competing hypotheses)
```
Users report the app exits after one message. Spawn 5 teammates to investigate
different hypotheses. Have them message each other to disprove each other's
theories — like a scientific debate. Update the findings doc with whatever
consensus emerges.
```

The debate structure breaks anchoring. The surviving theory is far more likely correct.

### Pattern C — Cross-layer build
```
Build the new notifications feature. Spawn a frontend teammate (owns src/ui/notifications/),
a backend teammate (owns src/api/notifications/), and a test teammate (owns tests/).
They must coordinate the API contract via the shared task list before implementing.
```

### Pattern D — Plan-gated risky work
```
Spawn an architect teammate to refactor the authentication module. Require plan
approval before they make any changes. Only approve plans that include test
coverage and do not modify the database schema.
```

The lead approves/rejects autonomously. **Give it explicit criteria** — that's how you control its judgment.

### Pattern E — Use a subagent definition as a teammate
```
Spawn a teammate using the security-reviewer agent type to audit the auth module.
```

The subagent definition's `tools` allowlist and `model` are honored; its body is **appended** to the system prompt (not replacing it). Coordination tools (`SendMessage`, task tools) are always available regardless of `tools` restrictions. Note: the definition's `skills` and `mcpServers` fields are **not** applied for teammates — they load skills/MCP from project + user settings like a normal session.

---

## 6. Operating the team

### Talking to teammates
- **In-process**: Shift+Down cycles teammates; type to send. Enter views their session; Esc interrupts. Ctrl+T toggles task list.
- **Split panes**: click into the pane.

### Task lifecycle
- States: pending → in-progress → completed.
- Dependencies: a task can't be claimed until its prerequisites complete.
- Claiming uses **file locking** to prevent races.
- Either the **lead assigns**, or teammates **self-claim** the next unblocked task.

### Plan approvals
- Teammate finishes plan → submits to lead.
- Lead approves (teammate exits plan mode and implements) or rejects with feedback (teammate revises and resubmits).
- Lead's judgment is steered by your spawn prompt — bake the criteria in.

### Shutting down a teammate
```
Ask the researcher teammate to shut down
```
Teammate may approve or reject with explanation.

### Cleaning up the team
```
Clean up the team
```
- **Always run cleanup from the lead.** Never from a teammate (their team context may not resolve, leaving inconsistent state).
- Cleanup **fails if any teammate is still running** — shut them down first.

---

## 7. Quality gates via hooks

Hooks enforce rules at coordination boundaries (exit code 2 = block + send feedback):

| Hook | Fires when | Block effect |
|---|---|---|
| `TeammateIdle` | Teammate about to go idle | Sends feedback, keeps teammate working |
| `TaskCreated` | Task being created | Prevents creation, sends feedback |
| `TaskCompleted` | Task being marked complete | Prevents completion, sends feedback |

Use these to enforce: required test runs, lint passes, doc updates, definition-of-done checks.

---

## 8. Permissions

- Teammates **inherit the lead's permission mode at spawn**.
- If the lead runs with `--dangerously-skip-permissions`, every teammate does too.
- Per-teammate permissions can be changed **after** spawn but **not at spawn**.
- Permission requests from teammates **bubble up to the lead** — pre-approve common ops in `permissions` to reduce friction.

---

## 9. Context and communication — what teammates do and don't see

Each teammate, on spawn:
- ✅ Loads project CLAUDE.md, MCP servers, skills (from project + user settings)
- ✅ Receives the spawn prompt from the lead
- ❌ Does **not** see the lead's conversation history

Implication: **put task-specific facts in the spawn prompt**. Don't assume context transfers.

Example — load the prompt with what the teammate needs:
```
Spawn a security reviewer teammate with the prompt: "Review src/auth/ for security
vulnerabilities. Focus on token handling, session management, input validation.
The app uses JWT tokens stored in httpOnly cookies. Report issues with severity
ratings."
```

Communication primitives:
- **Direct messages** — by teammate name; one message per recipient (no broadcast).
- **Idle notifications** — teammates auto-notify the lead when finished.
- **Task list** — shared visibility of all work.

---

## 10. Token economics

- Each teammate is a **separate context window** consuming tokens independently.
- Cost scales **roughly linearly** with team size.
- Worth it for: research, multi-angle review, parallel implementation of independent modules, debate-driven debugging.
- Not worth it for: routine fixes, single-file edits, sequential work.
- For long teams, watch for redundant work — synthesize and merge findings periodically rather than letting all teammates run unchecked.

---

## 11. Common pitfalls and fixes

| Symptom | Likely cause | Fix |
|---|---|---|
| Teammates don't appear | In-process mode hides them | Shift+Down to cycle; or check task was complex enough to spawn |
| Split panes fail | tmux/iTerm2 not installed/configured | `which tmux`; install or switch to in-process mode |
| Lead does work itself | Anchoring on the task | Tell it: "Wait for your teammates to complete their tasks before proceeding" |
| Teammates step on the same file | Bad task partitioning | Reassign so each teammate owns a disjoint file set |
| Permission prompt storm | Bubble-up from teammates | Pre-approve common ops in `permissions` |
| Teammate stops on error | Didn't recover | Shift+Down → instruct directly, or spawn a replacement |
| Lead shuts down team early | Premature "done" judgment | Tell it to keep going; check tasks aren't actually complete |
| Stuck task | Teammate didn't mark complete | Verify work is done; manually update or nudge teammate |
| Orphan tmux session | Cleanup didn't fully run | `tmux ls` → `tmux kill-session -t <name>` |
| `/resume` after team ran | In-process teammates not restored | Lead may message ghost teammates — tell it to spawn new ones |

---

## 12. Hard limits to remember

- **One team per session.** Clean up before creating another.
- **No nested teams.** Teammates cannot spawn their own teams.
- **Lead is fixed.** No leadership transfer.
- **`/resume` and `/rewind` do not restore in-process teammates.**
- **Shutdown is not instant** — teammates finish current tool call/request first.
- **No per-teammate permission mode at spawn time.**

---

## 13. Pre-flight checklist

Before spawning a team, confirm:

- [ ] The work has **≥3 truly independent strands** (or competing hypotheses).
- [ ] No two strands need to edit the same file.
- [ ] I can articulate a **distinct lens / role** for each teammate.
- [ ] I know how I'll **synthesize results** at the end.
- [ ] Risky work has **plan-approval gates** with explicit criteria.
- [ ] Common permissions are **pre-approved** to avoid prompt storms.
- [ ] Quality gates (hooks) are in place if relevant.
- [ ] The token cost is justified vs. a single session or subagents.
- [ ] I've written the spawn prompt with **per-teammate context** — not assuming history transfers.
- [ ] I know how to clean up: shut down teammates, then `Clean up the team` from the lead.

---

## 14. Reference prompt skeletons

**Skeleton — research/review team**
```
Create an agent team to {goal}. Spawn N teammates:
- {role 1}: {lens, files, deliverable}
- {role 2}: ...
Each works independently, then shares findings via direct messages.
Synthesize and produce {final artifact}.
```

**Skeleton — debate team**
```
Spawn N teammates to investigate {problem}. Each owns a different hypothesis:
- {teammate}: {hypothesis}
They must message each other to attack each other's theories. Update {findings doc}
with the surviving consensus.
```

**Skeleton — parallel build**
```
Build {feature}. Spawn teammates with disjoint file ownership:
- frontend: owns {paths}
- backend: owns {paths}
- tests: owns {paths}
Coordinate the {contract/interface} via the shared task list before implementing.
```

**Skeleton — gated refactor**
```
Spawn an architect teammate to refactor {module}. Require plan approval.
Only approve plans that {criteria}. Reject plans that {anti-criteria}.
After approval, spawn implementor teammates with disjoint file ownership.
```

---

## 15. Relationship to other parallelism tools

| Mechanism | Coordination | Cross-talk | Best for |
|---|---|---|---|
| Single session | N/A | N/A | Sequential / same-file work |
| Subagents | Main agent only | None | Focused result, low token cost |
| **Agent team** | Shared task list + mailbox | Direct messaging | Parallel work needing discussion |
| Git worktrees | Manual | None (separate sessions) | Hands-on multi-branch work |

---

*Keep this file current. When you discover a new failure mode, fix, or pattern, add it to the relevant section.*
