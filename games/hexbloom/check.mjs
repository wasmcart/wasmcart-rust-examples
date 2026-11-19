// Hexbloom's checks, run by scripts/check-carts.mjs on the Node host.
//
// Real pads prove the controls (A starts, the left stick moves, the right
// stick fires, any seat plays). The long run to the first goal uses the
// cart's own autopilot, switched on through its debug table, so the same
// policy runs here and in romdev in one ranged call.
export default async function ({ file, shots, boot, check, control, pad, idle, changed, countWhere, black, savePng }) {
  const mint = (f) => countWhere(f, (r, g, b) => g > 220 && b > 180 && r > 110 && r < 190, { x: 280, y: 100, w: 400, h: 90 });
  const stick = (lx, ly, rx = 0, ry = 0, buttons = 0) => pad(buttons, { lx, ly, rx, ry });

  // Title screen: the name, and it moves (the floor blooms by itself).
  {
    const cart = await boot(file);
    await cart.run(120, [idle(), idle()]);
    const title = await cart.grab();
    savePng(shots, 'title.png', title);
    check('hexbloom: title shows the name', mint(title) > 2500, `${mint(title)} px`);
    await cart.run(90, [idle(), idle()]);
    const title2 = await cart.grab();
    check('hexbloom: title screen is alive', changed(title, title2) > 200, `${changed(title, title2)} px changed`);
    control('the name check on a black frame', mint(black(title)) > 2500);
    control('the motion check of a frame against itself', changed(title, title) > 200);
    await cart.run(600, [idle(), idle()]);
    control('ten seconds of idle pads as a started game', cart.v('mode') !== 0, `mode ${cart.v('mode')}`);
    cart.destroy();
  }

  // Real pad input, seat 1 then seat 2 (the other seat idle).
  for (const seat of [0, 1]) {
    const cart = await boot(file);
    const on = (p) => (seat === 0 ? [p, idle()] : [idle(), p]);
    await cart.run(40, on(idle()));
    await cart.run(4, on(pad(1)));
    await cart.run(30, on(idle()));
    const started = cart.v('mode') === 1;
    const x0 = cart.v('px');
    await cart.run(50, on(stick(1, 0)));
    const moved = cart.v('px') - x0;
    // Sweep the right stick around for ten seconds while circling.
    await cart.run(600, (f) => on(stick(Math.cos(f / 40), Math.sin(f / 40), Math.cos(f / 15), Math.sin(f / 15))));
    const kills = cart.v('kills');
    check(`hexbloom: pad ${seat + 1} A starts the game`, started);
    check(`hexbloom: pad ${seat + 1} left stick moves the gardener`, moved > 100, `+${moved.toFixed(0)} px`);
    check(`hexbloom: pad ${seat + 1} right stick fires and kills`, kills > 3, `${kills} kills`);
    if (seat === 0) {
      const a = await cart.grab();
      await cart.run(10, on(stick(0, 1, 1, 0)));
      const b = await cart.grab();
      check('hexbloom: play moves between frames', changed(a, b) > 300, `${changed(a, b)} px`);
      const audio = cart.audioRms();
      check('hexbloom: sound reaches the audio ring', audio.rms > 0.01, `rms ${audio.rms.toFixed(4)}`);
    }
    cart.destroy();
  }
  {
    // Control: the same circling with the right stick left alone kills nothing.
    const cart = await boot(file);
    await cart.run(40, [idle(), idle()]);
    await cart.run(4, [pad(1), idle()]);
    await cart.run(680, (f) => [stick(Math.cos(f / 40), Math.sin(f / 40)), idle()]);
    control('moving without the right stick as firing', cart.v('kills') > 3, `${cart.v('kills')} kills`);
    await cart.run(2, [idle(), idle()]);
    const before = await boot(file);
    await before.run(2, []);
    const quiet = before.audioRms();
    control('the first frames, before any sound has loaded, as sound', quiet.rms > 0.01, `rms ${quiet.rms.toFixed(4)}`);
    before.destroy();
    cart.destroy();
  }

  // The first goal, by the autopilot, with an idle pad in each seat.
  {
    const cart = await boot(file);
    await cart.run(40, [idle(), idle()]);
    cart.host.writeDebugValue('autopilot', 1);
    let shot1 = false, shot2 = false, wave4 = 0;
    while (cart.frame < 7000 && cart.v('first_bloom') !== 1) {
      await cart.run(20, [idle(), idle()]);
      if (!shot1 && cart.v('wave') === 2 && cart.v('enemies') >= 6) { shot1 = true; savePng(shots, 'play-1.png', await cart.grab()); }
      if (cart.v('wave') === 4) wave4++;
      if (!shot2 && wave4 === 21) { shot2 = true; savePng(shots, 'play-2.png', await cart.grab()); }
    }
    check('hexbloom: the first thorn flower (wave 4) can be felled', cart.v('first_bloom') === 1, `frame ${cart.frame}, lives ${cart.v('lives')}, score ${cart.v('score')}`);
    const f = await cart.grab();
    const tinted = (fr) => countWhere(fr, (r, g, b) => Math.max(r, g, b) - Math.min(r, g, b) > 25 && Math.max(r, g, b) > 30, { x: 24, y: 56, w: 912, h: 468 });
    check('hexbloom: the floor blooms where enemies fell', tinted(f) > 20000, `${tinted(f)} tinted px`);
    control('the bloom check on a black frame', tinted(black(f)) > 20000);
    cart.destroy();
    const off = await boot(file);
    await off.run(40, [idle(), idle()]);
    off.host.writeDebugValue('autopilot', 0);
    await off.run(900, [idle(), idle()]);
    control('the autopilot left off as a played game', off.v('mode') !== 0 || off.v('wave') > 0, `mode ${off.v('mode')}`);
    off.destroy();
  }
}
