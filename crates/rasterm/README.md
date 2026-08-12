# rasterm

`rasterm` is a terminal-UI-independent raster graphics seam. It validates owned RGBA images, fits them to cell geometry, detects a presentation protocol, encodes native terminal graphics, and retains image lifecycle state. It has no Ratatui or Intuigram model dependency.

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

Native byte output is written through a caller-provided `Write`; the crate does not own a terminal. Its stateful renderer owns the optional external helper and temporary-image lifecycle. Encoding and helper work may block, so TUI consumers must call it outside their input and draw loop. Consumers can always use `text_cells` while an external result is unavailable.

The detector follows the same broad priority and environment signals described by [Yazi's image-preview documentation][yazi-preview]: `$TERM`, `$TERM_PROGRAM`, then graphical/external fallbacks.

## Multiplexers

tmux commands are DCS-wrapped with escaped `ESC` bytes. Users must enable `allow-passthrough` and preserve `TERM` and `TERM_PROGRAM`. Legacy cursor-anchored Kitty placement is intentionally not selected for tmux.

Zellij is represented explicitly and leaves native sequences unwrapped. Stable Zellij releases historically exposed Sixel; [Zellij main added Kitty graphics support][zellij-changelog] after 0.44.3. A Zellij build containing that change uses cursor-anchored Kitty placement when `TERM_PROGRAM` identifies a compatible Ghostty, kitty, or Konsole host. Zellij does not support Kitty Unicode placeholders, so `rasterm` never selects them through that multiplexer. Other Zellij hosts keep the Sixel fallback and require Sixel support in both Zellij and the outer terminal.

## Alacritty

Alacritty exposes no native inline-image protocol. On X11/Wayland, install Überzug++ and execute the generated layer commands outside the UI thread. On other platforms, install Chafa and execute the generated argv asynchronously, or use the built-in half-block cells.

## Licensing

`rasterm` is original code under the workspace's `MIT OR Apache-2.0` license. The research reference, [Yazi], is MIT-licensed and is not linked or copied. Überzug++ (GPL-3.0) and Chafa (LGPL-3.0-or-later) are optional external programs; `rasterm` only describes commands and does not distribute or link them.

[yazi]: https://github.com/sxyazi/yazi
[yazi-preview]: https://yazi-rs.github.io/docs/image-preview/
[zellij-changelog]: https://github.com/zellij-org/zellij/blob/main/CHANGELOG.md
