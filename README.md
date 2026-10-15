# Game Super Runtime workspace

This workspace is ready for a long-running AI development project. No application
or technology stack has been chosen yet.

Give the next agent your detailed project prompt. The agent should preserve the
assignment, establish milestones, and maintain a small checkpoint as it works.
The files support continuity across context compactions and fresh sessions;
they cannot guarantee automatic recovery unless the agent reads and updates them.

## Starting a project

Use this preface with your detailed prompt:

> Read AGENTS.md and notes/STATE.md. The project instructions follow. Preserve
> them in notes/BRIEF.md, investigate the workspace, and establish verifiable
> milestones. Work through the authorized scope, validating results and keeping
> the checkpoint current so another session can continue from it.

Then paste your project instructions, constraints, examples, and definition of
done. Unknowns are fine; the agent should investigate and make assumptions
explicit.

## Resuming work

> Read AGENTS.md and notes/STATE.md, then the referenced brief, decisions, and
> relevant files. Verify the actual workspace state and continue from the next
> action. Ask for clarification only if the recorded information is insufficient
> for a material decision.

## Workspace notes

| File | Purpose |
| --- | --- |
| [AGENTS.md](AGENTS.md) | Agent working instructions and recovery procedure |
| [Brief](notes/BRIEF.md) | Assignment, constraints, milestones, acceptance criteria |
| [State](notes/STATE.md) | Current checkpoint and next executable actions |
| [Decisions](notes/DECISIONS.md) | Consequential choices and their rationale |
| [Work log](notes/WORK_LOG.md) | Milestone evidence, investigations, and handoffs |

Add topic notes only when needed and link them from the checkpoint. Update this
README with actual installation, run, and validation commands when the project
has an implementation.
