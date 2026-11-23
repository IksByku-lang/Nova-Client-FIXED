//! Nova's HUD in the frame (`horizon_client::nova_hud` says what it is): drawn over the world
//! while playing, with the debug overlay closed, in the corner the launcher chose.
use super::{Game, Screen};
use horizon_client::{debug_overlay, hud::HudBatch, hud_layout, nova_hud};

impl Game<'_> {
    pub(super) fn draw_nova_hud(&self, batch: &mut HudBatch, size: [u32; 2]) {
        let Some(config) = nova_hud::config() else {
            return;
        };
        // Only in the world: not over menus, not with F1 (hide GUI), not beside F3's own lines.
        if nova_hud::hidden()
            || self.title.is_some()
            || self.screen != Screen::Playing
            || self.ui.hide_gui
            || self.debug
        {
            return;
        }
        let position = self
            .player
            .as_ref()
            .map(|p| [p.position.x as f64, p.position.y as f64, p.position.z as f64]);
        let memory = self
            .memory_meter
            .borrow_mut()
            .lines()
            .map(|[first, _, _]| first);
        let lines = config.lines(self.fps, memory.as_deref(), position);
        if lines.is_empty() {
            return;
        }
        let framebuffer = size.map(|v| v as i32);
        let (scale, [width, _]) = debug_overlay::scale(
            framebuffer,
            batch.scale as i32,
            self.settings.file.int("debugGuiScale"),
            self.settings.force_unicode,
            [batch.width as i32, batch.height as i32],
        );
        let factor = config.size.factor();
        let (scale, width) = (scale * factor, (width as f32 / factor) as i32);
        let none: Vec<String> = Vec::new();
        let columns = match config.corner {
            nova_hud::Corner::TopLeft => [lines.as_slice(), none.as_slice()],
            nova_hud::Corner::TopRight => [none.as_slice(), lines.as_slice()],
        };
        let draws = debug_overlay::draw(&self.gui.atlas.font, columns, scale, width);
        hud_layout::paint(batch, &self.gui.atlas, &draws);
    }
}
