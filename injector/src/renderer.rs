//! Graphics API selection for the cleaner window.
//!
//! The window is created inside somebody else's program, so the renderer is
//! picked before anything is created and cannot be retried afterwards: winit
//! allows one event loop per process, and every `build()` after the first one
//! fails no matter why the first one failed. [`choose`] therefore probes the
//! graphics backends *without* a window (which is possible for wgpu, not for
//! OpenGL) and returns a single, final answer.
//!
//! * OpenGL goes through `glow`/`glutin`, the smallest and most compatible
//!   option.
//! * DirectX means DirectX 12 through wgpu, which is the only DirectX backend wgpu
//!   exposes.
//! * Vulkan goes through wgpu as well.

use crate::config::Backend;
use crate::log;

/// The renderer eframe should use.
pub struct Choice {
    pub renderer: eframe::Renderer,
    /// Which wgpu backend to use when `renderer` is `Wgpu`.
    pub wgpu_backends: Option<eframe::wgpu::Backends>,
    /// Name for the log.
    pub name: &'static str,
}

/// A renderer plus the wgpu backend behind it.
#[derive(Clone)]
struct Candidate {
    renderer: eframe::Renderer,
    wgpu_backends: Option<eframe::wgpu::Backends>,
}

/// Picks the renderer for `backend`.
///
/// An explicit backend that this machine has no adapter for falls back to the
/// automatic chain instead of failing: the user asked for a specific API, but a
/// window that never comes up helps nobody.
pub fn choose(backend: Backend) -> Choice {
    let chain: Vec<(&'static str, Candidate)> = match backend {
        Backend::Auto => available(&[
            ("opengl", open_gl()),
            ("directx-12", direct_x()),
            ("vulkan", vulkan()),
        ]),
        Backend::OpenGl => available(&[("opengl", open_gl())]),
        Backend::DirectX => available(&[("directx-12", direct_x())]),
        Backend::Vulkan => available(&[("vulkan", vulkan())]),
    };

    let (name, chosen) = chain.into_iter().next().unwrap_or_else(|| {
        log::warn("no requested renderer is available on this machine");
        // Nothing was usable at all, which for OpenGL means there is no GL driver.
        // Let eframe try its own default and report the failure.
        (
            "eframe default",
            Candidate {
                renderer: eframe::Renderer::default(),
                wgpu_backends: None,
            },
        )
    });

    log::info(&format!("renderer: {name} ({})", chosen.renderer));
    Choice {
        renderer: chosen.renderer,
        wgpu_backends: chosen.wgpu_backends,
        name,
    }
}

/// Keeps the backends that are actually usable on this machine.
fn available(chain: &[(&'static str, Option<Candidate>)]) -> Vec<(&'static str, Candidate)> {
    chain
        .iter()
        .filter_map(|(name, candidate)| candidate.clone().map(|c| (*name, c)))
        .collect()
}

fn open_gl() -> Option<Candidate> {
    Some(Candidate {
        renderer: eframe::Renderer::Glow,
        wgpu_backends: None,
    })
}

#[cfg(windows)]
fn direct_x() -> Option<Candidate> {
    wgpu_candidate("DirectX 12", eframe::wgpu::Backends::DX12)
}

#[cfg(not(windows))]
fn direct_x() -> Option<Candidate> {
    // DirectX only exists on Windows.
    None
}

#[cfg(any(windows, target_os = "linux"))]
fn vulkan() -> Option<Candidate> {
    wgpu_candidate("Vulkan", eframe::wgpu::Backends::VULKAN)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn vulkan() -> Option<Candidate> {
    None
}

/// A wgpu backend, but only when an adapter for it actually exists.
#[cfg(any(windows, target_os = "linux"))]
fn wgpu_candidate(name: &str, backends: eframe::wgpu::Backends) -> Option<Candidate> {
    if !has_adapter(backends) {
        log::info(&format!("{name} has no adapter on this machine"));
        return None;
    }
    Some(Candidate {
        renderer: eframe::Renderer::Wgpu,
        wgpu_backends: Some(backends),
    })
}

/// Enumerates adapters without creating a window.
#[cfg(any(windows, target_os = "linux"))]
fn has_adapter(backends: eframe::wgpu::Backends) -> bool {
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = backends;
    let descriptor = eframe::wgpu::InstanceDescriptor {
        backends: setup.instance_descriptor.backends,
        flags: setup.instance_descriptor.flags,
        backend_options: setup.instance_descriptor.backend_options.clone(),
        memory_budget_thresholds: setup.instance_descriptor.memory_budget_thresholds,
        display: None,
    };
    let instance = eframe::wgpu::Instance::new(descriptor);
    // This runs on the window thread before the event loop exists, so it cannot
    // await: block here instead.
    !futures::executor::block_on(instance.enumerate_adapters(backends)).is_empty()
}

/// Applies the wgpu backend of a [`Choice`] to the eframe options.
pub fn apply(options: &mut eframe::NativeOptions, choice: &Choice) {
    let Some(backends) = choice.wgpu_backends else {
        return;
    };
    let mut configuration = eframe::egui_wgpu::WgpuConfiguration::default();
    let mut setup = eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle();
    setup.instance_descriptor.backends = backends;
    configuration.wgpu_setup = eframe::egui_wgpu::WgpuSetup::CreateNew(setup);
    options.wgpu_options = configuration;
}