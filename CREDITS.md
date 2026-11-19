# Credits and third-party content

Every file in this repository is either written for it or listed below with
its origin and licence. Nothing here is copied from a commercial game: the
games, their names, rules, levels, characters and look are original, except
Zemeroth, an open-source game ported here under its own licence (below).

## Sounds and music

Every WAV under `games/*/assets/` is generated from code by that game's
`gen-assets.mjs`, using the synthesizer in `scripts/lib/synth.mjs`. They are
original and dedicated to the public domain (CC0 1.0). Re-run the generator
to rebuild them.

## Pictures

- In-game graphics of the original games are drawn by the games' own code (macroquad shapes,
  wgpu meshes and shaders, a 5x7 bitmap font in Starwell's `font.rs`, Bevy
  meshes built from primitives and vertex colours in Hillfold and Lantern
  Links, Lantern Links' glow texture made in code). There are no image
  assets.
- Lantern Links' course, `games/lantern-links/assets/holes.txt`, was
  written by hand for this repository. Original, CC0 1.0.
- Launcher art under `games/*/art/` (icon, cover, background, logo, title
  screen, screenshots) is made by `scripts/make-art.mjs`: the title screen
  and screenshots are frames of the games themselves, and the icon, cover,
  background and logo are drawn on a canvas from code and those frames.
  Original, CC0 1.0.
- The art's lettering is set in DejaVu Sans Bold when it is installed
  (Bitstream Vera licence, DejaVu changes public domain); only the rendered
  pictures are in this repository, not the font.

## Fonts in the carts

The macroquad games draw text with macroquad's built-in font, ProggyClean
(Copyright (c) 2004, 2005 Tristan Grimmer, MIT licence), which macroquad
embeds. Starwell draws its own bitmap font, written for it. The Bevy games
use Bevy's default font (Bevy's `default_font` feature), Fira Mono
(Copyright (c) 2012-2015 The Mozilla Foundation and Telefonica S.A., SIL
Open Font License 1.1), which Bevy embeds.

## Libraries linked into the carts

- [macroquad](https://github.com/not-fl3/macroquad) and
  [miniquad](https://github.com/not-fl3/miniquad): MIT or Apache-2.0.
- [wasmcart-macroquad](https://github.com/wasmcart/wasmcart-macroquad): MIT.
- [wgpu](https://github.com/gfx-rs/wgpu): MIT or Apache-2.0.
- [wasmcart-rust](https://github.com/wasmcart/wasmcart-rust) (`wasmcart`,
  `wasmcart-wgpu`): MIT.
- [Bevy](https://github.com/bevyengine/bevy): MIT or Apache-2.0, with three
  of its crates as patched by
  [wasmcart-bevy](https://github.com/wasmcart/wasmcart-bevy) (MIT or
  Apache-2.0), which also links into the Bevy games.
- Their dependencies, as listed in `Cargo.lock`, under their own licences.

## Zemeroth (games/zemeroth)

- Zemeroth, by Andrey Lesnikov and contributors,
  https://github.com/ozkriff/zemeroth, master at
  fae7d89abe9702b25729453395e46fce105debbd: the game, its zgui, zscene and
  zcomponents crates, its data files (`assets/*.ron`) and its sprite atlas
  (`assets_src/atlas.svg`, the author's own art). MIT or Apache-2.0, at your
  option; see `games/zemeroth/LICENSE-MIT` and
  `games/zemeroth/LICENSE-APACHE`. The port's changes are under the same
  terms.
- `games/zemeroth/assets/img/*.png` are exported from that atlas by
  `games/zemeroth/utils/export-sprites.mjs` (with resvg); same terms.
- Zemeroth's text logo (`games/zemeroth/zemeroth.svg`, kept with the
  vendored source and not used by the cart) is based on the "Old London"
  font by Dieter Steffmann, as upstream's README says.
- Open Sans (`games/zemeroth/assets/OpenSans-Regular.ttf`), the game's
  font: Digitized data copyright 2010-2011, Google Corporation; Apache
  License 2.0.
- Karla (`games/zemeroth/zgui/assets`, `games/zemeroth/zscene/assets`), used
  only by those crates' examples, not by the cart: Copyright (c) 2011-2012,
  Jonathan Pinhorn; SIL Open Font License 1.1.
- Zemeroth's launcher art (`games/zemeroth/art`) is made by
  `scripts/make-art.mjs` like the other games', from the cart's own frames;
  its icon is drawn from three of the game's sprites, so it carries the
  sprites' MIT or Apache-2.0 terms.
