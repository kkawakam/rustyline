Know issues

## Document / Syntax

We would like to introduce an incremental parsing phase (see `tree-sitter`).
Because, when you have tokens (which may be as simple as words) or an AST,
completion / suggestion / highlighting / validation become easy.
So we need to send events to a lexer/parser, update `Document` accordingly.
And fix `Completer` / `Hinter` / `Highlighter` API such as they have access to `Document`.

See [lex_document](https://python-prompt-toolkit.readthedocs.io/en/master/pages/advanced_topics/rendering_flow.html#the-rendering-flow).

## Repaint / Refresh

Currently, performance is poor because, most of the time, we refresh the whole line (and prompt).
We would like to transform events on prompt/line/hint into partial repaint.

See `termwiz` design (`Surface`).

## Action / Command

We would like to support user defined actions that interact nicely with undo manager and kill-ring.
To do so, we need to refactor current key event dispatch.

See `replxx` design (`ACTION_RESULT`, `action_trait_t`).
