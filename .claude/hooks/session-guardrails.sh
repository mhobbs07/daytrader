#!/usr/bin/env bash
# =============================================================================
# UserPromptSubmit Hook: Session Guardrails
# Injects context reminders on every user prompt to keep the agent on track.
#
# Receives session context as JSON on stdin.
# =============================================================================

INPUT=$(cat)

# Inject architectural reminders into the session
jq -n '{
  systemMessage: "SESSION RULES ACTIVE:\n- One logical task per session. Do not mix unrelated changes.\n- Run cargo check after every significant code change.\n- Run ./scripts/verify_agent_work.sh before ANY commit.\n- Respect module boundaries declared in each src/<module>/CLAUDE.md.\n- Update BACKLOG.md when completing tasks.\n- All monetary values: Decimal/newtypes. No raw f64.\n- No .unwrap()/.expect()/panic!() in production code.\n- Every new public function needs a test."
}'

exit 0
