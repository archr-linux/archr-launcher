//! The wlr-layer-shell surfaces: mapped only while the pill or the battery
//! card is visible.
use crate::card::{self, Assets, Card};
use crate::pill::{self, Geometry, Painter, Timeline};
use crate::proto::{Battery, Icon, Message};
use smithay_client_toolkit::reexports::client::{
    globals::GlobalList,
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::time::{Duration, Instant};

struct Shown {
    layer: LayerSurface,
    configured: bool,
    timeline: Timeline,
    shown_at: Instant,
    changed_at: Instant,
    /// What the last committed frame shows: icon, fill edge and opacity.
    drawn: Option<(Icon, u32, u8)>,
}

struct CardShown {
    layer: LayerSurface,
    configured: bool,
    card: Card,
    hold: Duration,
    shown_at: Instant,
    changed_at: Instant,
    size: (u32, u32),
}

pub struct Overlay {
    registry: RegistryState,
    outputs: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: Option<SlotPool>,
    painter: Option<Painter>,
    shown: Option<Shown>,
    assets: Assets,
    card_pool: Option<SlotPool>,
    card: Option<CardShown>,
}

impl Overlay {
    pub fn new(globals: &GlobalList, qh: &QueueHandle<Self>) -> Result<Self, String> {
        let missing = |what| move |e| format!("{what} unavailable: {e}");
        Ok(Self {
            registry: RegistryState::new(globals),
            outputs: OutputState::new(globals, qh),
            compositor: CompositorState::bind(globals, qh).map_err(missing("wl_compositor"))?,
            layer_shell: LayerShell::bind(globals, qh).map_err(missing("wlr-layer-shell"))?,
            shm: Shm::bind(globals, qh).map_err(missing("wl_shm"))?,
            pool: None,
            painter: None,
            shown: None,
            assets: Assets::load(
                std::path::Path::new("/usr/share/archr-launcher"),
                std::path::Path::new("/storage/.config/system/configs/system.cfg"),
            ),
            card_pool: None,
            card: None,
        })
    }

    fn output_size(&self) -> (i32, i32) {
        let output = self.outputs.outputs().next();
        output
            .and_then(|o| self.outputs.info(&o))
            .and_then(|info| info.logical_size)
            .unwrap_or((640, 480))
    }

    pub fn show(&mut self, message: Message, qh: &QueueHandle<Self>) {
        let now = Instant::now();
        if let Some(shown) = &mut self.shown {
            let since = now - shown.changed_at;
            shown.timeline.retarget(message.icon, message.level, since);
            shown.changed_at = now;
            return;
        }
        let (w, h) = self.output_size();
        let geometry = Geometry::for_output(w, h);
        if self.painter.as_ref().map(|p| p.geometry) != Some(geometry) {
            self.painter = Some(Painter::new(geometry));
        }
        let surface = self.compositor.create_surface(qh);
        // An empty input region: touches and clicks go to whatever is below.
        if let Ok(region) = Region::new(&self.compositor) {
            surface.set_input_region(Some(region.wl_region()));
        }
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("archr-osd"),
            None,
        );
        layer.set_anchor(Anchor::RIGHT);
        layer.set_size(geometry.width, geometry.height);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(-1);
        layer.commit();
        self.shown = Some(Shown {
            layer,
            configured: false,
            timeline: Timeline::new(message.icon, message.level),
            shown_at: now,
            changed_at: now,
            drawn: None,
        });
    }

    /// The battery card, centered over the whole output; a new message
    /// replaces its text and keeps it up.
    pub fn show_card(&mut self, battery: Battery, qh: &QueueHandle<Self>) {
        let now = Instant::now();
        let card = self.assets.card(battery);
        let hold = if card.critical {
            card::HOLD
        } else {
            card::HOLD_LOW
        };
        if let Some(shown) = &mut self.card {
            shown.card = card;
            shown.hold = hold;
            shown.changed_at = now;
            return;
        }
        let (w, h) = self.output_size();
        let surface = self.compositor.create_surface(qh);
        if let Ok(region) = Region::new(&self.compositor) {
            surface.set_input_region(Some(region.wl_region()));
        }
        let layer = self.layer_shell.create_layer_surface(
            qh,
            surface,
            Layer::Overlay,
            Some("archr-osd"),
            None,
        );
        layer.set_size(w as u32, h as u32);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);
        layer.set_exclusive_zone(-1);
        layer.commit();
        self.card = Some(CardShown {
            layer,
            configured: false,
            card,
            hold,
            shown_at: now,
            changed_at: now,
            size: (w.max(1) as u32, h.max(1) as u32),
        });
    }

    fn card_tick(&mut self) -> Option<Duration> {
        let shown = self.card.as_ref()?;
        let now = Instant::now();
        let (since_shown, since_change) = (now - shown.shown_at, now - shown.changed_at);
        if since_change >= shown.hold + pill::FADE_OUT {
            self.card = None;
            return None;
        }
        const FRAME: Duration = Duration::from_millis(16);
        let next = if since_shown < pill::FADE_IN || since_change >= shown.hold {
            FRAME
        } else {
            shown.hold - since_change
        };
        if !shown.configured {
            return Some(next);
        }
        let fade_in = since_shown.as_secs_f32() / pill::FADE_IN.as_secs_f32();
        let fade_out = 1.0
            - since_change.saturating_sub(shown.hold).as_secs_f32() / pill::FADE_OUT.as_secs_f32();
        let frame = card::render(
            &shown.card,
            &self.assets,
            shown.size,
            fade_in.min(fade_out).clamp(0.0, 1.0),
        );
        let (w, h) = shown.size;
        let pool = match &mut self.card_pool {
            Some(pool) => pool,
            None => match SlotPool::new(frame.data().len(), &self.shm) {
                Ok(pool) => self.card_pool.insert(pool),
                Err(e) => {
                    eprintln!("archr-osd: shm pool: {e}");
                    return Some(next);
                }
            },
        };
        let Ok((buffer, canvas)) =
            pool.create_buffer(w as i32, h as i32, w as i32 * 4, wl_shm::Format::Argb8888)
        else {
            return Some(next);
        };
        for (dst, src) in canvas.chunks_exact_mut(4).zip(frame.data().chunks_exact(4)) {
            dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
        }
        let surface = shown.layer.wl_surface();
        surface.damage_buffer(0, 0, w as i32, h as i32);
        if buffer.attach_to(surface).is_ok() {
            shown.layer.commit();
        }
        Some(next)
    }

    /// Draws the current frames or unmaps finished surfaces; returns how
    /// long the event loop may sleep (None: until the next message).
    pub fn tick(&mut self) -> Option<Duration> {
        let card = self.card_tick();
        let pill = self.pill_tick();
        match (card, pill) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn pill_tick(&mut self) -> Option<Duration> {
        let shown = self.shown.as_mut()?;
        let now = Instant::now();
        let (since_shown, since_change) = (now - shown.shown_at, now - shown.changed_at);
        let Some(next) = pill::next_frame(since_shown, since_change) else {
            self.shown = None;
            return None;
        };
        if !shown.configured {
            return Some(next);
        }
        let painter = self.painter.as_ref()?;
        let g = painter.geometry;
        let (level, opacity) = (
            shown.timeline.level(since_change),
            pill::opacity(since_shown, since_change),
        );
        // The fill moves by whole pixels: most frames of a glide repeat the last one.
        let look = (
            shown.timeline.icon,
            g.fill_top(level) as u32,
            (opacity * 255.0) as u8,
        );
        if shown.drawn == Some(look) {
            return Some(next);
        }
        let frame = painter.paint(look.0, level, opacity);
        let stride = g.width as i32 * 4;
        let pool = match &mut self.pool {
            Some(pool) => pool,
            None => match SlotPool::new(frame.data().len(), &self.shm) {
                Ok(pool) => self.pool.insert(pool),
                Err(e) => {
                    eprintln!("archr-osd: shm pool: {e}");
                    return Some(next);
                }
            },
        };
        let format = wl_shm::Format::Argb8888;
        let Ok((buffer, canvas)) =
            pool.create_buffer(g.width as i32, g.height as i32, stride, format)
        else {
            return Some(next);
        };
        // tiny-skia is premultiplied RGBA; ARGB8888 is premultiplied BGRA in memory.
        for (dst, src) in canvas.chunks_exact_mut(4).zip(frame.data().chunks_exact(4)) {
            dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
        }
        let surface = shown.layer.wl_surface();
        surface.damage_buffer(0, 0, g.width as i32, g.height as i32);
        if buffer.attach_to(surface).is_ok() {
            shown.layer.commit();
            shown.drawn = Some(look);
        }
        Some(next)
    }
}

impl LayerShellHandler for Overlay {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if self.card.as_ref().is_some_and(|c| &c.layer == layer) {
            self.card = None;
        } else {
            self.shown = None;
        }
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        if let Some(shown) = self.shown.as_mut().filter(|s| &s.layer == layer) {
            shown.configured = true;
        }
        if let Some(shown) = self.card.as_mut().filter(|s| &s.layer == layer) {
            shown.configured = true;
        }
    }
}

impl CompositorHandler for Overlay {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for Overlay {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.outputs
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for Overlay {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Overlay {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState];
}

delegate_compositor!(Overlay);
delegate_output!(Overlay);
delegate_shm!(Overlay);
delegate_layer!(Overlay);
delegate_registry!(Overlay);
