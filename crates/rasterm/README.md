# rasterm

`rasterm` is a terminal-UI-independent raster graphics seam. It validates
owned RGBA images, fits them to cell geometry, detects a presentation protocol,
encodes native terminal graphics, and retains image lifecycle state. It has no
Ratatui or Intuigram model dependency.

## Protocols

| Environment | Selected presentation | Implementation |
| --- | --- | --- |
| Ghostty, kitty | Kitty Unicode placeholders | In-process RGBA transmission and virtual-placement lifecycle |
| Konsole | Legacy Kitty placement | In-process cursor-anchored RGBA transmission |
| iTerm2, WezTerm, Warp, Tabby, VS Code | iTerm2 inline image | In-process PNG and OSC 1337 encoding |
| foot, Sixel terminals, Windows Terminal | Sixel | In-process RGB332 Sixel encoding |
| X11/Wayland with `ueberzugpp` | Überzug++ | Shell-free JSON command descriptors for an asynchronous layer owner |
| `chafa` fallback | Chafa | Shell-free argv descriptors for an asynchronous caller |
| Everything else | Unicode half blocks | In-process RGBA sampling with alpha blending |

Native byte output is written through a caller-provided `Write`; the crate does
not own a terminal, launch a process, or block an event loop. External renderer
descriptors likewise leave process ownership, file lifetime, backpressure, and
cancellation to the caller. Consumers can always use `text_cells` while an
external result is unavailable.

The detector follows the same broad priority and environment signals described
by [Yazi's image-preview documentation][yazi-preview]: `$TERM`,
`$TERM_PROGRAM`, then graphical/external fallbacks. No Yazi source is copied.

## Multiplexers

tmux commands are DCS-wrapped with escaped `ESC` bytes. Users must enable
`allow-passthrough` and preserve `TERM` and `TERM_PROGRAM`, as documented by
Yazi. Legacy cursor-anchored Kitty placement is intentionally not selected for
tmux.

Zellij is represented explicitly and leaves native sequences unwrapped. Stable
Zellij releases historically exposed Sixel; [Zellij main added Kitty graphics
support][zellij-changelog] after 0.44.3. A Zellij build containing that change
can preserve the outer Ghostty/kitty capability through `TERM_PROGRAM` and use
Unicode placeholders. Older builds need Sixel support in both Zellij and the
outer terminal or the text fallback. This is deliberately documented because
Yazi's current stable documentation still describes Zellij as Sixel-only.

## Alacritty

Alacritty exposes no native inline-image protocol. On X11/Wayland, install
Überzug++ and execute the generated layer commands outside the UI thread. On
other platforms, install Chafa and execute the generated argv asynchronously,
or use the built-in half-block cells.

## Licensing

`rasterm` is original code under the workspace's `MIT OR Apache-2.0` license.
The research reference, [Yazi][yazi], is MIT-licensed and is not linked or
copied. Überzug++ (GPL-3.0) and Chafa (LGPL-3.0-or-later) are optional external
programs; `rasterm` only describes commands and does not distribute or link
them.

[yazi-preview]: https://yazi-rs.github.io/docs/image-preview/
[yazi]: https://github.com/sxyazi/yazi
[zellij-changelog]: https://github.com/zellij-org/zellij/blob/main/CHANGELOG.md
