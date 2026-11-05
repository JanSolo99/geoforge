# Working efficiently with agents

Start routine changes with your configured workhorse model. Clear evidence and
small feedback loops reduce retries; they do not guarantee identical results or
lower bills. Measure completed, validated tasks and rework, not just token price.
Use a stronger reasoning model for an unresolved semantic or architectural question
only when the evidence packet below makes that question concrete.

## Model selection

The shared coordinator accepts optional environment settings:

```sh
export UNIGIS_AGENTS_CLAUDE_MODEL=sonnet
# Set this to a workhorse model ID supported by your installed Codex CLI/account:
# export UNIGIS_AGENTS_CODEX_MODEL=YOUR_MODEL_ID
./scripts/agents
```

`/status` displays the requested model for each provider. Unset Claude/Codex values
preserve CLI defaults; Kimi retains `kimi-code/k3.1-pro`. Settings apply to new,
resumed and native `/cli` launches. They do not change CLI permissions, subscriptions
or provider fallback order. The displayed value is a request, not confirmation of
the provider's actual served model. Unsupported model errors must be corrected in
configuration; the coordinator does not silently upgrade to a premium model.
Restart the coordinator after changing its environment. After a major task change,
checkpoint the handoff and use `/new` to avoid carrying stale conversation history.

## Task brief

Use this short structure in a request or handoff; do not create a new planning file
for every small edit:

```text
Outcome: observable user behavior to deliver.
Evidence: reproduction or input -> expected output; source of compatibility truth.
Scope: likely files and existing example to follow; inherited edits to preserve.
Validation: targeted command, required gate, unavailable external checks.
Done when: concrete checks and deliverables; publication only if authorized.
Next action: one executable step.
```

For example: “Fix field decoding for this FileGDB fixture; preserve its three
SmallInteger rows. Reproduce the oracle test, verify native values and GDAL version,
then run the root gate. Do not change unrelated comparison tolerances.”

## Skills and context

Codex/project instructions point to `.agents/skills/`; Claude discovers wrappers in
`.claude/skills/`. The canonical skills are `unigis-delivery`, `unigis-ci-triage` and
`unigis-tool`. Read only the relevant skill. AGENTS.md and CLAUDE.md remain mandatory;
skills do not override nested app instructions or authorize deployment.

Read the handoff and actual working tree before resuming. Search large documents by
the current subsystem. Prefer one nearby implementation and its tests over reading
every registry. Expand context when a concrete dependency requires it. Keep durable
findings in project docs; replace stale handoff text instead of appending transcripts.
Periodic ambition review is optional and must be explicitly requested, not launched
automatically after every small change.

## Escalation packet

After two failed attempts at the same hypothesis, stop repeating it. Capture:

- Exact question or decision, desired behavior and compatibility source.
- Revision, relevant files, smallest input and reproduction command.
- Expected versus actual output and the first causal error.
- Hypotheses tried and what each result ruled out.
- Smallest proposed next experiment; independent work that can continue.

Missing credentials, ArcGIS access, build dependencies or memory need an environment
fix, not a more expensive model. Do not invent reference data or relax a gate. Keep
checks that were not run explicitly pending. The user should receive a focused
decision or blocker, not be asked to reconstruct the session.

## Validation

`scripts/check` includes offline coordinator tests using fake CLIs; no model calls
or provider charges are needed. To test just coordinator changes:

```sh
python3 -m unittest discover -s tools/agent-coordinator -p 'test_*.py' -v
```

The full root gate and applicable app gates still apply. Use targeted checks while
iterating, then run the required gate once the coherent change is ready. Record the
revision and outcome so a later agent does not repeat a costly build without cause.
