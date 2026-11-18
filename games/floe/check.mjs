// Bumper Floe checks: pad bots play the cart on the Node host.
//
// 1. One human (pad 0, a bot) with a connected pad 1 that is never touched:
//    pad 1 must not join, the match must not stall, and the bot reaches the
//    match end screen. A run with no input must stay on the lobby.
// 2. Two humans join (pads 0 and 1) and both bumpers move their own way.
// 3. Pixels: the lobby and the floe draw; play moves between frames.
// 4. Audio reaches the ring.
// Each check has a control that must fail.

const MODE = { LOBBY: 0, COUNTDOWN: 1, PLAY: 2, ROUND_END: 3, MATCH_END: 4 };

/** The seat 0 bot: stay on the ice, chase the nearest rival, charge and dash into it. */
function botPad(c, BTN, pad) {
  const mode = c.v('mode');
  if (mode !== MODE.PLAY || !c.v('p0alive')) return pad(0);
  const x = c.v('p0x'), y = c.v('p0y'), vx = c.v('p0vx'), vy = c.v('p0vy');
  const cx = c.v('cx'), cy = c.v('cy');
  const margin = c.v('p0margin');
  const charge = c.v('p0charge');
  const tox = cx - x, toy = cy - y;
  const tl = Math.hypot(tox, toy) || 1;
  const outward = -(vx * tox + vy * toy) / tl;
  if (margin < 60 + Math.max(0, outward) * 0.25) {
    // Back to the middle, braking while still sliding out.
    return pad(outward > 20 ? BTN.B : 0, { lx: tox / tl, ly: toy / tl });
  }
  const tx = c.v('tx'), ty = c.v('ty');
  if (tx < 0) return pad(0, { lx: tox / tl, ly: toy / tl });
  const dx = tx - x, dy = ty - y, d = Math.hypot(dx, dy) || 1;
  const charging = charge > 0;
  if (charging && charge < 0.45) return pad(BTN.A, { lx: dx / d, ly: dy / d });
  if (charging) return pad(0, { lx: dx / d, ly: dy / d }); // release: dash
  if (d < 150 && c.frame % 12 === 0) return pad(BTN.A, { lx: dx / d, ly: dy / d });
  return pad(0, { lx: dx / d, ly: dy / d });
}

export default async function (ctx) {
  const { boot, check, control, pad, idle, BTN, grab, savePng, changed, black, colours, countWhere, name, file, shots } = ctx;
  void grab;
  const press = (b) => pad(b);

  // ── 1. one human, an idle second pad, to the match end ──
  {
    const c = await boot(file);
    await c.run(40, [idle(), idle()]);
    const lobbyShot = await c.grab();
    check(`${name}: idle pads do not join`, c.v('humans') === 0 && c.v('mode') === MODE.LOBBY, `humans ${c.v('humans')} mode ${c.v('mode')}`);
    // Join with A, then Start.
    await c.run(3, [press(BTN.A), idle()]);
    await c.run(10, [idle(), idle()]);
    await c.run(30, [idle(), idle()]);
    const joinedShot = await c.grab();
    savePng(shots, 'title.png', joinedShot);
    check(`${name}: pad 0 A joins seat P1 only`, c.v('humans') === 1, `humans mask ${c.v('humans')}`);
    await c.run(3, [press(BTN.START), idle()]);
    check(`${name}: Start begins the match`, c.v('mode') === MODE.COUNTDOWN, `mode ${c.v('mode')}`);
    c.audioRms();
    let maxRound = 0, p1Joined = false, maxHits = 0, p0Moved = 0, lastX = c.v('p0x');
    let playShot = null, audio = null, roundsWon = 0;
    const f0 = c.frame;
    while (c.frame - f0 < 30000 && c.v('mode') !== MODE.MATCH_END) {
      await c.run(1, (fr, cc) => [botPad(cc, BTN, pad), idle()]);
      maxRound = Math.max(maxRound, c.v('round'));
      if (c.v('humans') !== 1) p1Joined = true;
      maxHits = Math.max(maxHits, c.v('hits'));
      if (c.v('mode') === MODE.PLAY) { p0Moved += Math.abs(c.v('p0x') - lastX); }
      lastX = c.v('p0x');
      if (!playShot && c.v('mode') === MODE.PLAY && c.v('round') === 1 && c.v('hits') >= 4 && c.v('floe_r') < 221.5 && c.v('alive') >= 3) {
        playShot = await c.grab();
        savePng(shots, 'play-1.png', playShot);
        await c.run(6, (fr, cc) => [botPad(cc, BTN, pad), idle()]);
        const later = await c.grab();
        check(`${name}: play moves between frames`, changed(playShot, later) > 300, `${changed(playShot, later)} px changed`);
        control('a frame compared with itself as motion', changed(playShot, playShot) > 300);
        const ice = (f) => countWhere(f, (r, g, b) => b > 225 && g > 210 && r > 180 && r < 215);
        check(`${name}: the floe draws`, ice(playShot) > 40000, `${ice(playShot)} ice px`);
        control('a black frame as the floe', ice(black(playShot)) > 40000);
      }
      if (!audio && c.frame - f0 > 600) audio = c.audioRms();
    }
    roundsWon = c.v('wins0');
    const fr = c.frame - f0;
    check(`${name}: one human reaches the match end`, c.v('mode') === MODE.MATCH_END,
      `mode ${c.v('mode')} after ${fr} frames (${(fr / 3600).toFixed(1)} min), ${maxRound} rounds, P1 won ${roundsWon}, winner seat ${c.v('match_winner')}`);
    check(`${name}: the idle second pad never joined or stalled it`, !p1Joined && maxRound >= 3, `rounds ${maxRound}`);
    check(`${name}: the P1 bot moves its bumper`, p0Moved > 500, `${p0Moved.toFixed(0)} px travelled`);
    check(`${name}: bumpers collide`, maxHits >= 5, `${maxHits} hits`);
    check(`${name}: audio reaches the ring`, audio && audio.rms > 0.01, audio ? `rms ${audio.rms.toFixed(4)} over ${audio.samples} samples` : 'none');
    control('silence as audio', (() => { const z = new Float32Array(48000); let s = 0; for (const v of z) s += v * v; return Math.sqrt(s / z.length) > 0.01; })());
    check(`${name}: the lobby draws`, colours(lobbyShot) > 40, `${colours(lobbyShot)} colours`);
    control('a black frame as the lobby', colours(black(lobbyShot)) > 40);
    // The end screen restarts on A.
    await c.run(90, [idle(), idle()]);
    const endShot = await c.grab();
    savePng(shots, 'match-end.png', endShot);
    await c.run(3, [press(BTN.A), idle()]);
    check(`${name}: A on the end screen starts a rematch`, c.v('mode') === MODE.COUNTDOWN && c.v('round') === 1 && c.v('wins0') === 0, `mode ${c.v('mode')} round ${c.v('round')}`);
    c.destroy();
  }

  // ── no input: the lobby stays ──
  {
    const c = await boot(file);
    await c.run(900, [idle(), idle(), idle(), idle()]);
    control('a run with no input as a started game', c.v('mode') !== MODE.LOBBY || c.v('humans') !== 0, `mode ${c.v('mode')} humans ${c.v('humans')}`);
    c.destroy();
  }

  // ── 2. two humans ──
  {
    const c = await boot(file);
    await c.run(20, [idle(), idle()]);
    await c.run(3, [press(BTN.A), press(BTN.A)]);
    await c.run(10, [idle(), idle()]);
    check(`${name}: two pads join two seats`, c.v('humans') === 3, `humans mask ${c.v('humans')}`);
    await c.run(3, [idle(), press(BTN.START)]);
    while (c.v('mode') !== MODE.PLAY && c.frame < 600) await c.run(1, [idle(), idle()]);
    // P2 waits while P1 moves: an untouched pad must not move its seat.
    const iy = c.v('p1y');
    await c.run(20, [pad(0, { lx: 1 }), idle()]);
    const idleDy = c.v('p1y') - iy;
    const x0 = c.v('p0x'), x1 = c.v('p1x'), y0 = c.v('p0y'), y1 = c.v('p1y');
    // P1 starts top left, P2 top right: P1 slides right, P2 slides down.
    await c.run(20, [pad(0, { lx: 1 }), pad(BTN.DOWN)]);
    const d0 = c.v('p0x') - x0, d1 = c.v('p1y') - y1;
    check(`${name}: P1 moves right on its stick`, d0 > 30, `dx ${d0.toFixed(0)}`);
    check(`${name}: P2 moves down on its d-pad`, d1 > 30, `dy ${d1.toFixed(0)}`);
    check(`${name}: each pad drives only its own seat`, Math.abs(c.v('p0y') - y0) < d0 / 2 && Math.abs(c.v('p1x') - x1) < d1 / 2,
      `P1 dy ${(c.v('p0y') - y0).toFixed(0)}, P2 dx ${(c.v('p1x') - x1).toFixed(0)}`);
    // Both charge and dash into the middle.
    await c.run(30, [pad(BTN.A, { lx: 0.7, ly: 0.7 }), pad(BTN.A, { lx: -0.7, ly: 0.7 })]);
    await c.run(8, [pad(0, { lx: 0.7, ly: 0.7 }), pad(0, { lx: -0.7, ly: 0.7 })]);
    const shot = await c.grab();
    savePng(shots, 'play-2.png', shot);
    control('an untouched P2 pad as P2 moving', idleDy > 30, `dy ${idleDy.toFixed(0)}`);
    c.destroy();
  }
}
