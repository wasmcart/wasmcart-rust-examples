//! Gamepad play for the battle screen (this port's addition).
//!
//! A hex cursor moves over the map with the d-pad or the left stick and A
//! clicks the tile under it, exactly as the mouse does. B cancels, X cycles
//! the selected fighter's abilities, Y ends the turn, L and R jump between
//! your fighters, Select inspects the unit under the cursor and Start opens
//! the menu.

use std::time::Duration;

use mq::math::Vec2;
use ui::{self, Gui};

use crate::{
    assets,
    core::{
        battle::{state, Id, PlayerId, State},
        map::{self, PosHex},
    },
    geom,
    input::Pad,
    screen::{
        self,
        battle::{build_panel_agent_info, Battle, Message, SelectionMode},
        Screen, StackCommand,
    },
    utils::line_heights,
    ZResult,
};

impl Battle {
    pub(super) fn set_cursor(&mut self, pos: PosHex) {
        self.cursor = pos;
        self.cursor_anchor_x = geom::hex_to_point(self.view.tile_size(), pos).x;
    }

    /// Moves the cursor to the neighbouring tile that best matches the
    /// pressed direction. Straight up or down zigzags between the two
    /// upper (lower) neighbours, keeping to the column the cursor started in.
    pub(super) fn move_cursor(&mut self, (dx, dy): (i32, i32)) {
        let size = self.view.tile_size();
        let from = geom::hex_to_point(size, self.cursor);
        let want = Vec2::new(dx as f32, dy as f32).normalize_or_zero();
        let mut best: Option<(f32, PosHex)> = None;
        for dir in map::dirs() {
            let pos = map::Dir::get_neighbor_pos(self.cursor, dir);
            if !self.state.map().is_inboard(pos) {
                continue;
            }
            let point = geom::hex_to_point(size, pos);
            let mut score = (point - from).normalize_or_zero().dot(want);
            if dx == 0 {
                score -= (point.x - self.cursor_anchor_x).abs() / size * 0.01;
            }
            if score > 0.3 && best.map_or(true, |(s, _)| score > s) {
                best = Some((score, pos));
            }
        }
        if let Some((_, pos)) = best {
            let anchor = self.cursor_anchor_x;
            self.set_cursor(pos);
            if dx == 0 {
                self.cursor_anchor_x = anchor;
            }
            self.view.show_current_tile_marker(pos);
        }
    }

    pub(super) fn handle_pad(&mut self, pad: &Pad) -> ZResult<StackCommand> {
        if let Some(dir) = pad.dir {
            self.move_cursor(dir);
        }
        if pad.start {
            return Ok(StackCommand::PushPopup(self.popup_confirm_exit()?));
        }
        if pad.select {
            if let Some(id) = state::agent_id_at_opt(&self.state, self.cursor) {
                let popup = UnitInfo::new(&self.state, id)?;
                return Ok(StackCommand::PushPopup(Box::new(popup)));
            }
        }
        if self.block_timer.is_some() {
            return Ok(StackCommand::None);
        }
        if pad.a {
            self.handle_click_pos(self.cursor)?;
            self.view.messages_map_mut().clear();
        }
        if pad.b {
            match (self.selected_agent_id, self.mode) {
                (Some(id), SelectionMode::Ability(_)) => {
                    self.set_mode(id, SelectionMode::Normal)?
                }
                (Some(_), SelectionMode::Normal) => self.deselect()?,
                _ => {}
            }
        }
        if pad.x {
            self.pad_next_ability()?;
        }
        if pad.y && self.panel_end_turn.is_some() && self.mode == SelectionMode::Normal {
            self.end_turn()?;
        }
        if pad.l || pad.r {
            self.pad_next_agent(if pad.r { 1 } else { -1 })?;
        }
        Ok(StackCommand::None)
    }

    /// X: the selected fighter's next ability, then back to moving.
    fn pad_next_ability(&mut self) -> ZResult {
        let id = match self.selected_agent_id {
            Some(id) => id,
            None => match state::agent_id_at_opt(&self.state, self.cursor) {
                Some(id) => {
                    self.set_mode(id, SelectionMode::Normal)?;
                    id
                }
                None => return Ok(()),
            },
        };
        let abilities: Vec<_> = match self.state.parts().abilities.get_opt(id) {
            Some(a) => a.0.iter().map(|r| r.ability).collect(),
            None => Vec::new(),
        };
        let next = match self.mode {
            SelectionMode::Ability(current) => abilities
                .iter()
                .position(|&a| a == current)
                .and_then(|i| abilities.get(i + 1))
                .copied(),
            SelectionMode::Normal => abilities.first().copied(),
        };
        match next {
            Some(ability) => self.set_mode(id, SelectionMode::Ability(ability)),
            None if abilities.is_empty() => {
                self.view.message(self.cursor, "no abilities")?;
                Ok(())
            }
            None => self.set_mode(id, SelectionMode::Normal),
        }
    }

    /// L / R: select the previous / next of your fighters.
    fn pad_next_agent(&mut self, step: i32) -> ZResult {
        let ids = state::players_agent_ids(&self.state, PlayerId(0));
        if ids.is_empty() {
            return Ok(());
        }
        let n = ids.len() as i32;
        let next = match self
            .selected_agent_id
            .and_then(|id| ids.iter().position(|&i| i == id))
        {
            Some(i) => (i as i32 + step).rem_euclid(n),
            None if step > 0 => 0,
            None => n - 1,
        };
        let id = ids[next as usize];
        let pos = self.state.parts().pos.get(id).0;
        self.set_cursor(pos);
        self.view.show_current_tile_marker(pos);
        self.set_mode(id, SelectionMode::Normal)
    }

    /// A living fighter of yours (the selected one may have just died).
    fn is_own(&self, id: Id) -> bool {
        self.state.parts().agent.get_opt(id).is_some()
            && state::is_agent_belong_to(&self.state, PlayerId(0), id)
    }

    pub(super) fn pad_prompts(&self) -> Vec<screen::Prompt> {
        if self.panel_end_turn.is_none() && self.mode == SelectionMode::Normal {
            return vec![("Wait", "the enemy moves")];
        }
        match (self.selected_agent_id, self.mode) {
            (Some(_), SelectionMode::Ability(_)) => {
                vec![("A", "use here"), ("X", "next ability"), ("B", "cancel")]
            }
            (Some(id), SelectionMode::Normal) if self.is_own(id) => vec![
                ("A", "move / attack"),
                ("X", "ability"),
                ("B", "cancel"),
                ("L/R", "fighter"),
                ("Select", "inspect"),
                ("Y", "end turn"),
            ],
            _ => vec![
                ("A", "select"),
                ("L/R", "fighter"),
                ("Select", "inspect"),
                ("Y", "end turn"),
                ("Start", "menu"),
            ],
        }
    }

    /// Debug values for checks and the autopilot.
    pub(super) fn publish_debug(&self) {
        use wasmcart_macroquad::debug;
        debug::set_i32("cursor_q", self.cursor.q);
        debug::set_i32("cursor_r", self.cursor.r);
        let selected = self
            .selected_agent_id
            .and_then(|id| self.state.parts().pos.get_opt(id))
            .map(|p| p.0);
        debug::set_i32("selected", selected.is_some() as i32);
        debug::set_i32("selected_q", selected.map_or(0, |p| p.q));
        debug::set_i32("selected_r", selected.map_or(0, |p| p.r));
        let ability = matches!(self.mode, SelectionMode::Ability(_));
        debug::set_i32("ability_mode", ability as i32);
        debug::set_i32("blocked", self.block_timer.is_some() as i32);
        debug::set_i32("can_end_turn", self.panel_end_turn.is_some() as i32);
        let own = state::players_agent_ids(&self.state, PlayerId(0)).len();
        let enemies = state::enemy_agent_ids(&self.state, PlayerId(0)).len();
        debug::set_i32("own_agents", own as i32);
        debug::set_i32("enemy_agents", enemies as i32);
    }
}

/// Select on a unit: its live stats, with info buttons for its passive
/// abilities and effects, as a popup a gamepad can navigate.
#[derive(Debug)]
pub struct UnitInfo {
    gui: Gui<Message>,
}

impl UnitInfo {
    fn new(state: &State, id: Id) -> ZResult<Self> {
        let mut gui = Gui::new();
        let middle = ui::Anchor(ui::HAnchor::Middle, ui::VAnchor::Middle);
        build_panel_agent_info(&mut gui, state, id, middle)?;
        let font = &assets::get().font;
        let text = ui::Drawable::text("back", font);
        let h = line_heights().big;
        let back = ui::Button::new(text, h, gui.sender(), Message::Back)?.default_focus();
        let layout = ui::VLayout::from_widget(Box::new(back));
        let bottom = ui::Anchor(ui::HAnchor::Middle, ui::VAnchor::Bottom);
        gui.add(&ui::pack(layout), bottom);
        Ok(Self { gui })
    }
}

impl Screen for UnitInfo {
    fn update(&mut self, _: Duration) -> ZResult<StackCommand> {
        Ok(StackCommand::None)
    }

    fn draw(&self) -> ZResult {
        self.gui.draw();
        Ok(())
    }

    fn click(&mut self, pos: Vec2) -> ZResult<StackCommand> {
        match self.gui.click(pos) {
            Some(Message::Back) => Ok(StackCommand::Pop),
            Some(Message::PassiveAbilityInfo(ability)) => {
                let popup = screen::GeneralInfo::new(&ability.title(), &ability.description())?;
                Ok(StackCommand::PushPopup(Box::new(popup)))
            }
            Some(Message::LastingEffectInfo(effect)) => {
                let popup = screen::GeneralInfo::new(&effect.title(), &effect.description())?;
                Ok(StackCommand::PushPopup(Box::new(popup)))
            }
            _ => Ok(StackCommand::None),
        }
    }

    fn resize(&mut self, aspect_ratio: f32) {
        self.gui.resize_if_needed(aspect_ratio);
    }

    fn move_mouse(&mut self, pos: Vec2) -> ZResult {
        self.gui.move_mouse(pos);
        Ok(())
    }

    fn pad(&mut self, pad: &Pad) -> ZResult<StackCommand> {
        if pad.b || pad.select {
            return Ok(StackCommand::Pop);
        }
        match screen::menu_nav(&mut self.gui, pad) {
            Some(pos) => self.click(pos),
            None => Ok(StackCommand::None),
        }
    }

    fn kind(&self) -> i32 {
        4
    }
}
