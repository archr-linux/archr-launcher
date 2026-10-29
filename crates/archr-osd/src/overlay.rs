//! The wlr-layer-shell surface: mapped only while the pill is visible.
use crate::pill::{self, Geometry, Painter, Timeline};
use crate::proto::Message;
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
        });
    }

    /// Draws the current frame or unmaps a finished pill; returns how long
    /// the event loop may sleep (None: until the next message).
    pub fn tick(&mut self) -> Option<Duration> {
        let shown = self.shown.as_ref()?;
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
        let frame = painter.paint(
            shown.timeline.icon,
            shown.timeline.level(since_change),
            pill::opacity(since_shown, since_change),
        );
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
        }
        Some(next)
    }
}

impl LayerShellHandler for Overlay {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.shown = None;
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
