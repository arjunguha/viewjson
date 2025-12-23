# Agent Instructions

## Commit Messages

All commit messages should have detailed descriptions and state that they are co-authored by the name of the model used.

Conclude the commit message with the sequence of human-written prompts to the agent. This may challenge the ability of agents to copy text verbatim, so it may help to write out all prompts to a temporary file as they are written.

Example format:
```
Brief summary of changes

Detailed description of what was changed and why.

Co-authored-by: ModelName <model@example.com>

Prompts:
- First prompt from user
- Second prompt from user
- etc.
```

## General Directions

Before committing, if the user provides general directions that are not specific to the particular task, update AGENTS.md with those directions. This ensures that important guidelines and preferences are preserved for future reference.

- Absolutely do not install GTK from Homebrew.
