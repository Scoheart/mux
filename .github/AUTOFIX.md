# Automated issue repair

MUX uses a guarded automation chain:

1. `Quality monitor` runs on pushes, pull requests, manual dispatches, and every day at 09:17 Asia/Shanghai.
2. A default-branch or scheduled failure opens or updates one `ci-failure` Issue.
3. When Copilot dispatch is configured, that Issue is assigned to Copilot cloud agent.
4. Copilot works on a branch and creates a PR. It never writes directly to `main`.
5. When the PR is ready, `Notify repair review` requests review from `Scoheart`, adds `needs-review`, and posts an @mention.

Public Issue text is untrusted. An arbitrary contributor cannot start a repair: manual dispatch requires the repository owner to add the `autofix` label or run the workflow.

## One-time setup

1. Enable Copilot cloud agent for the account and this repository. Confirm `copilot-swe-agent` appears in the Issue assignee picker.
2. Create a fine-grained user token following GitHub's Copilot assignment API requirements: metadata read plus Actions, Contents, Issues, and Pull requests read/write access for `Scoheart/mux`.
3. Add the token as the repository Actions Secret `COPILOT_PAT`.
4. Run `Quality monitor` manually once. To test the repair path safely, create a small Issue and add the `autofix` label yourself.

Do not put a broad account token into `COPILOT_PAT`; scope it to this repository and rotate it independently.

Official references:

- https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/use-cloud-agent-via-the-api
- https://docs.github.com/en/copilot/how-tos/copilot-on-github/use-copilot-agents/review-copilot-output
