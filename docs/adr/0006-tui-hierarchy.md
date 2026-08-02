# TUI hierarchy

Intuigram navigation follows the Chat list → Active Chat → Active Message hierarchy instead of cycling focus among every visible region. Entering a Chat activates its Composer by default, Message targeting is an explicit temporary descent from the Composer, `Esc` ascends one level at a time, and Folders are switched only from the Chat-list level. This keeps ordinary typing ready, makes context-sensitive actions predictable, and avoids keyboard modes and Tab-based focus traversal at the cost of dedicated modifier bindings for lateral navigation.
