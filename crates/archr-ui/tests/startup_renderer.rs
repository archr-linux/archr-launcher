use archr_ui::render::Ui;

#[test]
fn gles_window_matches_renderer_without_changing_requested_flags() {
    std::env::set_var("SDL_VIDEODRIVER", "offscreen");
    std::env::remove_var("ARCHR_LAUNCHER_EXP");
    std::env::remove_var("SDL_RENDER_DRIVER");
    let ui = Ui::new(640, 480, true).unwrap();
    assert_eq!(ui.canvas.info().name, "opengles2");
    assert_ne!(
        ui.canvas.window().window_flags() & sdl2::sys::SDL_WindowFlags::SDL_WINDOW_OPENGL as u32,
        0
    );
    let video = ui.sdl.video().unwrap();
    let gl = video.gl_attr();
    assert_eq!(gl.context_profile(), sdl2::video::GLProfile::GLES);
    assert_eq!(gl.context_version(), (2, 0));
    assert_eq!(gl.depth_size(), 0);
    assert_eq!(gl.stencil_size(), 0);
    assert_eq!(gl.multisample_buffers(), 0);
    assert_eq!(gl.multisample_samples(), 0);
    drop(video);
    drop(ui);
    std::env::set_var("SDL_RENDER_DRIVER", "software");
    let ui = Ui::new(640, 480, true).unwrap();
    assert_eq!(
        ui.canvas.info().name,
        "software",
        "explicit driver overrides the GLES2 hint"
    );
    drop(ui);
    std::env::remove_var("SDL_RENDER_DRIVER");
    std::env::set_var("SDL_VIDEODRIVER", "dummy");
    let ui = Ui::new(640, 480, true).unwrap();
    assert_eq!(ui.canvas.info().name, "software");
    assert_eq!(
        ui.canvas.window().window_flags() & sdl2::sys::SDL_WindowFlags::SDL_WINDOW_OPENGL as u32,
        0
    );
}
