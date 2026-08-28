# Model Card Context Design

## Goal

Expose a Model Profile's context-window size directly on its library card so users can compare configured models without opening the inspector.

## Data contract

- Prefer the persisted `ModelProfile.context_window`; retain the existing models.dev metadata fallback.
- When a Provider model-list response includes `context_length`, selecting that model copies the value into the editable `context_window` field and the saved profile.
- A subsequent manual Model ID edit clears only the context value that was auto-filled by the previous catalog selection. A value manually entered by the user remains authoritative.
- Missing values remain hidden. Do not infer context from display names or Model IDs such as `[1m]`.

## Presentation

- Keep the Model ID as the primary secondary line.
- Render a quiet, non-interactive `Context 1M` / `上下文 1M` metadata chip beside it.
- Include the exact token count in the tooltip for precision and accessibility.
- Avoid a colorful status badge: context size is descriptive metadata, not state.

## Verification contract

- A card uses the persisted profile value ahead of catalog metadata and renders an explicit context label.
- Selecting a discovered model populates the context field.
- Replacing the selected model with a manually entered ID clears the stale auto-filled context.
- Existing profiles without a real value render no context metadata.

