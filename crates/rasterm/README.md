# rasterm

`rasterm` provides a raster graphics interface that does not depend on a terminal UI. It validates owned RGBA images, fits them to cell geometry, detects a presentation protocol, encodes native terminal graphics, and keeps image lifecycle state. It does not depend on Ratatui or the Intuigram model.

## Protocols

| Environment                             | Selected presentation      | Implementation                                                      |
| --------------------------------------- | -------------------------- | ------------------------------------------------------------------- |
| Ghostty, Konsole                        | Legacy Kitty placement     | In-process cursor-anchored RGBA transmission                        |
| kitty                                   | Kitty Unicode placeholders | In-process RGBA transmission and virtual-placement lifecycle        |
| iTerm2, WezTerm, Warp, Tabby, VS Code   | iTerm2 inline image        | In-process PNG and OSC 1337 encoding                                |
| foot, Sixel terminals, Windows Terminal | Sixel                      | In-process RGB332 Sixel encoding                                    |
| X11/Wayland with `ueberzugpp`           | Überzug++                  | Shell-free JSON command descriptors for an asynchronous layer owner |
| `chafa` fallback                        | Chafa                      | Shell-free argv descriptors for an asynchronous caller              |
| Everything else                         | Unicode half blocks        | In-process RGBA sampling with alpha blending                        |

The caller provides the `Write` target for native byte output. The crate does not own a terminal. Its stateful renderer owns the optional external helper and temporary-image lifecycle. Encoding and helper work can block. Therefore, TUI consumers must call it outside the input and draw loop. Consumers can always use `text_cells` when an external result is not available.

The detector uses the priority and environment signals in the [Yazi image-preview documentation][yazi-preview]. It checks `$TERM`, then `$TERM_PROGRAM`, and then graphical or external fallbacks.

## Multiplexers

`rasterm` wraps tmux commands in DCS and escapes `ESC` bytes. Users must enable `allow-passthrough`. They must also preserve `TERM` and `TERM_PROGRAM`. `rasterm` intentionally does not select legacy cursor-anchored Kitty placement for tmux.

`rasterm` represents Zellij explicitly and does not wrap native sequences. Stable Zellij releases historically provided Sixel. [Zellij main added Kitty graphics support][zellij-changelog] after version 0.44.3. A Zellij build with this change uses cursor-anchored Kitty placement when `TERM_PROGRAM` identifies a compatible Ghostty, kitty, or Konsole host. Zellij does not support Kitty Unicode placeholders. Therefore, `rasterm` never selects them through Zellij. Other Zellij hosts continue to use Sixel. Sixel support is necessary in Zellij and in the outer terminal.

## Alacritty

Alacritty does not provide a native inline-image protocol. On X11 or Wayland, install Überzug++ and execute the generated layer commands outside the UI thread. On other platforms, install Chafa and execute the generated argv asynchronously. As an alternative, use the built-in half-block cells.

## Licensing

`rasterm` is original code under the workspace `MIT OR Apache-2.0` license. The research reference, [Yazi], uses the MIT license. `rasterm` does not link or copy Yazi. Überzug++ uses GPL-3.0. Chafa uses LGPL-3.0-or-later. They are optional external programs. `rasterm` only describes commands. It does not distribute or link these programs.

[yazi]: https://github.com/sxyazi/yazi
[yazi-preview]: https://yazi-rs.github.io/docs/image-preview/
[zellij-changelog]: https://github.com/zellij-org/zellij/blob/main/CHANGELOG.md
