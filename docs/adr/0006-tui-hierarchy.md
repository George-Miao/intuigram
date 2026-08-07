# TUI hierarchy

Intuigram navigation follows the Chat list → Active Chat → Active Message hierarchy instead of cycling focus among every visible region. Entering a Chat activates its Composer by default, Message targeting is an explicit temporary descent from the Composer, `Esc` ascends one level at a time, and Folders are switched only from the Chat-list level with bare `Left` and `Right` (plus `Alt` compatibility aliases). This keeps ordinary typing ready and makes context-sensitive actions predictable without keyboard modes or Tab-based focus traversal.
