# Credits and third-party content

Every file in this repository is either written for it or listed below with
its origin and licence. Nothing here is copied from a commercial game: the
games, their names, rules, levels, characters and look are original.

## Sounds and music

Every WAV under `games/*/assets/` is generated from code by that game's
`gen-assets.mjs`, using the synthesizer in `scripts/lib/synth.mjs`. They are
original and dedicated to the public domain (CC0 1.0). Re-run the generator
to rebuild them.

## Pictures

- In-game graphics are drawn by the games' own code (macroquad shapes,
  wgpu meshes and shaders, a 5x7 bitmap font in Starwell's `font.rs`).
  There are no image assets.
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
embeds. Starwell draws its own bitmap font, written for it.

## Libraries linked into the carts

- [macroquad](https://github.com/not-fl3/macroquad) and
  [miniquad](https://github.com/not-fl3/miniquad): MIT or Apache-2.0.
- [wasmcart-macroquad](https://github.com/wasmcart/wasmcart-macroquad): MIT.
- [wgpu](https://github.com/gfx-rs/wgpu): MIT or Apache-2.0.
- [wasmcart-rust](https://github.com/wasmcart/wasmcart-rust) (`wasmcart`,
  `wasmcart-wgpu`): MIT.
- Their dependencies, as listed in `Cargo.lock`, under their own licences.
