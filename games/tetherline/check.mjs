// Tetherline's checks, run by scripts/check-carts.mjs on the Node host.
//
// Real pads prove the controls (A starts and jumps, the d-pad runs, holding B
// near a ring tethers, a second seat plays too). The long runs use the cart's
// own autopilot, switched on through its debug table.
export default async function ({ file, shots, boot, check, control, pad, idle, BTN, changed, countWhere, black, savePng }) {
  const cream = (f) => countWhere(f, (r, g, b) => r > 235 && g > 225 && b > 200, { x: 230, y: 90, w: 500, h: 90 });

  {
    const cart = await boot(file);
    await cart.run(120, [idle(), idle()]);
    const title = await cart.grab();
    savePng(shots, 'title.png', title);
    check('tetherline: title shows the name', cream(title) > 3000, `${cream(title)} px`);
    await cart.run(60, [idle(), idle()]);
    const t2 = await cart.grab();
    check('tetherline: title screen is alive (clouds drift)', changed(title, t2) > 100, `${changed(title, t2)} px`);
    control('the name check on a black frame', cream(black(title)) > 3000);
    control('the motion check of a frame against itself', changed(title, title) > 100);
    await cart.run(600, [idle(), idle()]);
    control('ten seconds of idle pads as a started game', cart.v('mode') !== 0, `mode ${cart.v('mode')}`);
    cart.destroy();
  }

  for (const seat of [0, 1]) {
    const cart = await boot(file);
    const on = (b) => (seat === 0 ? [pad(b), idle()] : [idle(), pad(b)]);
    await cart.run(40, on(0));
    await cart.run(4, on(BTN.A));
    await cart.run(20, on(0));
    const started = cart.v('mode') === 1;
    const x0 = cart.v('px'), y0 = cart.v('py');
    await cart.run(40, on(BTN.RIGHT));
    const ran = cart.v('px') - x0;
    let minY = 1e9;
    await cart.run(2, on(BTN.A));
    for (let i = 0; i < 20; i++) { await cart.run(1, on(BTN.A)); minY = Math.min(minY, cart.v('py')); }
    await cart.run(30, on(0));
    check(`tetherline: pad ${seat + 1} A starts`, started);
    check(`tetherline: pad ${seat + 1} d-pad runs`, ran > 100, `+${ran.toFixed(0)} px`);
    check(`tetherline: pad ${seat + 1} A jumps`, y0 - minY > 60, `${(y0 - minY).toFixed(0)} px up`);
    if (seat === 0) {
      // Run right holding B: the line catches the first ring once it is in reach.
      let caught = false, a, b;
      for (let i = 0; i < 400 && !caught; i++) {
        await cart.run(1, on(BTN.RIGHT | BTN.B | (cart.v('gap') < 20 || cart.v('spikes') < 45 ? BTN.A : 0)));
        caught = cart.v('tethered') === 1;
      }
      check('tetherline: holding B tethers to the marked ring', caught, `at x ${Math.round(cart.v('px'))}, falls ${cart.v('deaths')}`);
      a = await cart.grab();
      await cart.run(8, on(BTN.RIGHT | BTN.B));
      b = await cart.grab();
      savePng(shots, 'play-1.png', b);
      check('tetherline: the swing moves', changed(a, b) > 2000, `${changed(a, b)} px`);
      const audio = cart.audioRms();
      check('tetherline: sound reaches the audio ring', audio.rms > 0.01, `rms ${audio.rms.toFixed(4)}`);
    }
    cart.destroy();
  }
  {
    const cart = await boot(file);
    await cart.run(40, [idle()]);
    await cart.run(4, [pad(BTN.A)]);
    let caught = false;
    for (let i = 0; i < 400; i++) { await cart.run(1, [pad(BTN.RIGHT | (cart.v('gap') < 20 || cart.v('spikes') < 45 ? BTN.A : 0))]); caught ||= cart.v('tethered') === 1; }
    control('running without B as a tether', caught);
    cart.destroy();
    const q = await boot(file);
    await q.run(2, []);
    const quiet = q.audioRms();
    control('the first frames, before any sound has loaded, as sound', quiet.rms > 0.01, `rms ${quiet.rms.toFixed(4)}`);
    q.destroy();
  }

  // The first goal (island 1's flag), then all five, by the autopilot.
  {
    const cart = await boot(file);
    await cart.run(40, [idle(), idle()]);
    cart.host.writeDebugValue('autopilot', 1);
    let firstFlag = -1, shot2 = false;
    while (cart.frame < 12000 && cart.v('mode') !== 6) {
      await cart.run(30, [idle(), idle()]);
      if (firstFlag < 0 && cart.v('level') >= 1) firstFlag = cart.frame;
      if (!shot2 && cart.v('level') === 2 && cart.v('px') > 1500) { shot2 = true; savePng(shots, 'play-2.png', await cart.grab()); }
    }
    check('tetherline: island 1 can be finished', firstFlag > 0, `flag by frame ${firstFlag}`);
    check('tetherline: all five islands can be finished', cart.v('mode') === 6, `frame ${cart.frame}, falls ${cart.v('deaths')}, score ${cart.v('score')}`);
    cart.destroy();
    const off = await boot(file);
    await off.run(40, [idle(), idle()]);
    off.host.writeDebugValue('autopilot', 0);
    await off.run(1200, [idle(), idle()]);
    control('the autopilot left off as a finished island', off.v('level') > 0 || off.v('mode') !== 0, `mode ${off.v('mode')}`);
    off.destroy();
  }
}
