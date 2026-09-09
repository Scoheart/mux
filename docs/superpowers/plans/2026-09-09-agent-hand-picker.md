# Agent hand picker implementation plan

**Goal:** Replace the Agent navigation dropdown with the approved compact hand interaction, preserving real Agent selection, pin persistence, and custom Agent creation.

**Architecture:** AgentNavigation owns the existing usePinnedAgents state and header trigger. PinnedAgentDock renders both the header shortcuts and the modal's interactive pin targets. AgentHandPicker owns grouping, search, paging, transient highlight/replacement state, and cancellable Web Animations. Reuse the global Modal focus/inert contract; open AddAgentDialog through the existing callback after the add card expands.

**Tech stack:** React, TypeScript, CSS transforms/opacity, Web Animations API; no new dependencies or backend changes.

- [x] Replace the old vertical picker with a compact portal hand. Preserve selected Agent only in header; focus search and do not preselect a card. Show 3–7 cards by available width, search all eligible Agents, and retain built-in/custom/pinned categories.
- [x] Add PinnedAgentDock with six slots in the picker, native drag reorder and Option+Left/Right keyboard reorder. Persist through usePinnedAgents with its existing optimistic rollback/error toast. A full dock enters inline replacement mode; Escape cancels replacement first.
- [x] Deal cards sequentially with 28 ms stagger / 250 ms travel; collect in reverse with 12 ms stagger / 150 ms travel. Hover and keyboard focus lift 26 px. Select exits in 160 ms without waiting for Agent detail data. Pin animates to a dock target; add expands before opening the existing form. Cancel all animations on unmount and honor reduced motion.
- [x] Review source diff for keyboard focus, click-through, small-window fit, no stale hover after paging, and persistent pin behavior. Keep backend and user config untouched. Per user instruction, do not run tests. Keep/start the authorized local development app for manual user review; do not publish.

Outcome: source review complete; TypeScript and Vite production compilation passed. The existing local Tauri process serves the updated frontend on localhost:1432. No tests run, no publication, and no real pin/Agent mutations performed by the assistant.

## Approved entry-transition refinement

- [x] Replace the short exit with a 190ms lift/1.9× enlargement followed by a 240ms shared-title landing. Start real navigation at the initial click and retain the existing modal as an input shield until completion.
- [x] Mark AgentHeader identity endpoints; animate the proxy icon/name into their measured positions, reveal the real title, and bring configuration/resource sections in with a small rise. Preload the view module when opening the hand.
- [x] Use a bounded fade if the target title is not ready. Reduced motion enters immediately; resize, backgrounding, exceptions and unmount clean up animations and hidden title state. Do not wait for backend data or persist any configuration.
- [x] Move picker ResizeObserver writes to a coalesced animation frame and avoid unchanged dock position updates. The inline prototype observer was separately fixed with outer-width observation, size guards and deferred repositioning.
