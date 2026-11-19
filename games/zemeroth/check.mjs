// Zemeroth's checks, run by scripts/check-carts.mjs on the Node host.
//
// Every press here is a real host pad press (or a real pointer event): the
// menus, the campaign, the hex cursor, and the whole first campaign battle,
// won on pad 0 with an idle pad 1 connected. For the battle the cart's
// autopilot runs in advisor mode (autopilot = 2): it publishes the one
// press it would make as `ap_bits`, and this script presses exactly that on
// pad 0. The `seed` switch fixes the dice so the run is the same each time.
// Each check has a control that must fail.

const SCREEN = { MENU: 1, CAMPAIGN: 2, BATTLE: 3, UNIT_INFO: 4, CONFIRM: 5, AGENT_INFO: 7 };
const SEED = 11;

export default async function ({ file, shots, boot, check, control, pad, idle, BTN, changed, colours, countWhere, black, savePng }) {
  const both = (p) => [p, idle()];
  const tap = async (c, b, after = 12) => {
    await c.run(2, both(pad(b)));
    await c.run(after, both(idle()));
  };
  // The prompt bar's orange key boxes along the top edge.
  const promptPx = (f) => countWhere(f, (r, g, b) => r > 200 && g > 95 && g < 140 && b < 40, { x: 300, y: 0, w: 680, h: 45 });
  const toMenu = async (c) => {
    for (let i = 0; i < 400 && c.v('screen') !== SCREEN.MENU; i += 10) await c.run(10, both(idle()));
  };

  // ── Main menu, campaign screen, info popups: pad only ──
  {
    const c = await boot(file);
    await toMenu(c);
    const menu = await c.grab();
    savePng(shots, 'title.png', menu);
    check('zemeroth: the main menu draws', c.v('screen') === SCREEN.MENU && colours(menu) > 12, `screen ${c.v('screen')}, ${colours(menu)} colours`);
    control('a black frame as the menu', colours(black(menu)) > 12);
    check('zemeroth: pad prompts show on the menu', promptPx(menu) > 200, `${promptPx(menu)} px`);
    control('the prompt check on a black frame', promptPx(black(menu)) > 200);
    await tap(c, BTN.A, 20);
    check('zemeroth: A on the menu opens the campaign', c.v('screen') === SCREEN.CAMPAIGN, `screen ${c.v('screen')}`);
    // Up from "Start battle": the info button of the nearest fighter.
    await tap(c, BTN.UP);
    await tap(c, BTN.A);
    check('zemeroth: d-pad focus and A open a fighter\'s info', c.v('screen') === SCREEN.AGENT_INFO, `screen ${c.v('screen')}`);
    savePng(shots, 'agent-info.png', await c.grab());
    await tap(c, BTN.B);
    check('zemeroth: B closes the info popup', c.v('screen') === SCREEN.CAMPAIGN, `screen ${c.v('screen')}`);
    await tap(c, BTN.START);
    check('zemeroth: Start on the campaign asks to abandon it', c.v('screen') === SCREEN.CONFIRM, `screen ${c.v('screen')}`);
    await tap(c, BTN.A);
    check('zemeroth: the confirm box focuses "no" first', c.v('screen') === SCREEN.CAMPAIGN, `screen ${c.v('screen')}`);
    c.destroy();
  }
  {
    // Control: without the d-pad, A starts the battle instead of opening info.
    const c = await boot(file);
    await toMenu(c);
    await tap(c, BTN.A, 20);
    await tap(c, BTN.A, 30);
    control('A without the d-pad as opening a fighter\'s info', c.v('screen') === SCREEN.AGENT_INFO, `screen ${c.v('screen')}`);
    await c.run(600, both(idle()));
    const c2 = await boot(file);
    await c2.run(900, both(idle()));
    control('fifteen seconds of idle pads as leaving the menu', c2.v('screen') !== SCREEN.MENU, `screen ${c2.v('screen')}`);
    c2.destroy();
    c.destroy();
  }

  // ── Mouse and touch still work ──
  {
    const c = await boot(file);
    await toMenu(c);
    // The campaign button, below the middle of the 1280x720 frame.
    c.host.pointerMove(0, 640, 394);
    c.host.pointerDown(0, 640, 394, 0);
    await c.run(2, both(idle()));
    c.host.pointerUp(0, 0);
    await c.run(20, both(idle()));
    check('zemeroth: a mouse click opens the campaign', c.v('screen') === SCREEN.CAMPAIGN, `screen ${c.v('screen')}`);
    const hidden = promptPx(await c.grab());
    check('zemeroth: using the mouse hides the pad prompts', hidden < 20, `${hidden} prompt px`);
    await tap(c, BTN.DOWN);
    const shown = promptPx(await c.grab());
    check('zemeroth: a pad press brings them back', shown > 200, `${shown} prompt px`);
    c.destroy();
    const t = await boot(file);
    await toMenu(t);
    t.host.pointerDown(1, 640, 394, 0);
    await t.run(2, both(idle()));
    t.host.pointerUp(1, 0);
    await t.run(20, both(idle()));
    check('zemeroth: a touch opens the campaign', t.v('screen') === SCREEN.CAMPAIGN, `screen ${t.v('screen')}`);
    t.destroy();
    const n = await boot(file);
    await toMenu(n);
    n.host.pointerMove(0, 640, 120);
    n.host.pointerDown(0, 640, 120, 0);
    await n.run(2, both(idle()));
    n.host.pointerUp(0, 0);
    await n.run(20, both(idle()));
    control('a click on empty space as opening the campaign', n.v('screen') === SCREEN.CAMPAIGN, `screen ${n.v('screen')}`);
    n.destroy();
  }

  // ── The first campaign battle, won on the pad ──
  const playBattle = async (c, pressAdvice) => {
    let f = 0;
    let moving = null;
    let midShot = false;
    while (c.v('screen') !== SCREEN.CAMPAIGN && f < 30000) {
      const bits = c.v('ap_bits') || 0;
      if (pressAdvice && bits) {
        await c.run(1, both(pad(bits)));
        await c.run(1, both(idle()));
        f += 2;
      } else {
        await c.run(1, both(idle()));
        f += 1;
      }
      if (!midShot && c.v('enemy_agents') === 2 && c.v('can_end_turn') === 1 && c.v('blocked') === 0) {
        midShot = true;
        savePng(shots, 'play-1.png', await c.grab());
      }
      if (moving === null && c.v('can_end_turn') === 0 && c.v('blocked') === 1) {
        // The enemy's turn: imps walk and strike.
        const a = await c.grab();
        await c.run(30, both(idle()));
        const b = await c.grab();
        moving = changed(a, b);
        check('zemeroth: the enemy turn moves between frames', moving > 300, `${moving} px changed`);
        savePng(shots, 'battle.png', b);
      }
    }
    return f;
  };
  {
    const c = await boot(file);
    await toMenu(c);
    c.host.writeDebugValue('seed', SEED);
    await tap(c, BTN.A, 20);
    await tap(c, BTN.A, 60);
    check('zemeroth: A on "Start battle" opens the first battle', c.v('screen') === SCREEN.BATTLE, `screen ${c.v('screen')}`);
    // The hex cursor on the d-pad.
    const q0 = c.v('cursor_q');
    await tap(c, BTN.RIGHT);
    const q1 = c.v('cursor_q');
    await tap(c, BTN.LEFT);
    check('zemeroth: the d-pad moves the hex cursor', q1 === q0 + 1 && c.v('cursor_q') === q0, `q ${q0} -> ${q1} -> ${c.v('cursor_q')}`);
    await tap(c, BTN.R);
    check('zemeroth: R selects a fighter', c.v('selected') === 1);
    const before = await c.grab();
    await tap(c, BTN.X);
    check('zemeroth: X picks an ability', c.v('ability_mode') === 1);
    const ability = await c.grab();
    savePng(shots, 'play-2.png', ability);
    check('zemeroth: the ability shows its tiles and description', changed(before, ability) > 2000, `${changed(before, ability)} px changed`);
    control('a frame compared with itself as a change', changed(before, before) > 2000);
    await tap(c, BTN.B);
    check('zemeroth: B leaves the ability', c.v('ability_mode') === 0 && c.v('selected') === 1);
    await tap(c, BTN.SELECT);
    check('zemeroth: Select inspects the unit under the cursor', c.v('screen') === SCREEN.UNIT_INFO, `screen ${c.v('screen')}`);
    savePng(shots, 'unit-info.png', await c.grab());
    await tap(c, BTN.B);
    await tap(c, BTN.B);
    c.host.writeDebugValue('autopilot', 2);
    const frames = await playBattle(c, true);
    await c.run(10, both(idle()));
    const won = c.v('screen') === SCREEN.CAMPAIGN && c.v('last_result') === 1;
    check('zemeroth: the first battle is won with pad presses', won,
      `screen ${c.v('screen')}, result ${c.v('last_result')}, ${c.v('own_agents')} fighters left, ${frames} frames (${(frames / 3600).toFixed(1)} min)`);
    check('zemeroth: the campaign advances to battle 2', c.v('campaign_battle') === 1 && c.v('campaign_mode') === 0,
      `battle index ${c.v('campaign_battle')}, mode ${c.v('campaign_mode')}, renown ${c.v('campaign_renown')}`);
    await c.run(30, both(idle()));
    savePng(shots, 'campaign-2.png', await c.grab());
    c.destroy();
  }
  {
    // Control: the advisor running but nobody pressing its presses.
    const c = await boot(file);
    await toMenu(c);
    c.host.writeDebugValue('seed', SEED);
    await tap(c, BTN.A, 20);
    await tap(c, BTN.A, 60);
    c.host.writeDebugValue('autopilot', 2);
    for (let i = 0; i < 3000; i += 60) await c.run(60, both(idle()));
    control('an advisor nobody follows as winning the battle', c.v('screen') !== SCREEN.BATTLE || c.v('enemy_agents') < 3,
      `screen ${c.v('screen')}, enemies ${c.v('enemy_agents')}`);
    c.destroy();
  }
}
