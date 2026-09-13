//! Load a game `cdylib` that exported [`crate::export_play_app!`].

use std::ffi::CString;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use libloading::{Library, Symbol};
use wiimaker_core::draw::DrawList;
use wiimaker_core::input::Input;
use wiimaker_core::world::World;

use crate::abi::{PlayInputC, PLAY_ABI_VERSION};

type AbiVersionFn = unsafe extern "C" fn() -> u32;
type CreateFn = unsafe extern "C" fn(
    *const std::os::raw::c_char,
    *const std::os::raw::c_char,
) -> *mut std::os::raw::c_void;
type DestroyFn = unsafe extern "C" fn(*mut std::os::raw::c_void);
type UpdateFn =
    unsafe extern "C" fn(*mut std::os::raw::c_void, *const PlayInputC, f32, u32, u32) -> i32;
type RenderFn = unsafe extern "C" fn(
    *mut std::os::raw::c_void,
    *mut DrawList,
    *const PlayInputC,
    f32,
    u32,
    u32,
) -> i32;
type WorldFn = unsafe extern "C" fn(*mut std::os::raw::c_void) -> *mut World;

/// `libhello_orb.so` / `libhello_orb.dylib` / `hello_orb.dll`
pub fn dylib_name(package: &str) -> String {
    format!(
        "{}{}{}",
        std::env::consts::DLL_PREFIX,
        package.replace('-', "_"),
        std::env::consts::DLL_SUFFIX
    )
}

pub fn target_dir(workspace: &Path) -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_TARGET_DIR") {
        return PathBuf::from(dir);
    }
    workspace.join("target")
}

pub fn dylib_path(workspace: &Path, package: &str) -> PathBuf {
    target_dir(workspace)
        .join("debug")
        .join(dylib_name(package))
}

pub fn example_dylib_path(workspace: &Path, example: &str) -> PathBuf {
    target_dir(workspace)
        .join("debug")
        .join("examples")
        .join(dylib_name(example))
}

/// Loaded play plugin. The `Library` must outlive `handle`.
pub struct LoadedPlugin {
    // Drop handle before library.
    handle: *mut std::os::raw::c_void,
    abi: u32,
    #[allow(dead_code)]
    create: CreateFn,
    destroy: DestroyFn,
    update: UpdateFn,
    render: RenderFn,
    world: WorldFn,
    // Kept so symbols stay mapped.
    _lib: Library,
}

// Plugin ticks only on the editor UI thread.
unsafe impl Send for LoadedPlugin {}

impl Drop for LoadedPlugin {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { (self.destroy)(self.handle) };
            self.handle = std::ptr::null_mut();
        }
    }
}

impl LoadedPlugin {
    pub fn abi(&self) -> u32 {
        self.abi
    }

    /// Peek ABI without creating a session (dylib must exist).
    pub fn abi_only(path: &Path) -> Result<u32> {
        let lib =
            unsafe { Library::new(path) }.with_context(|| format!("dlopen {}", path.display()))?;
        let abi_fn: Symbol<AbiVersionFn> = unsafe { lib.get(b"wiimaker_play_abi_version") }
            .context("missing wiimaker_play_abi_version")?;
        Ok(unsafe { abi_fn() })
    }

    pub fn load(path: &Path, game_dir: &Path, scene_json: Option<&str>) -> Result<Self> {
        let lib =
            unsafe { Library::new(path) }.with_context(|| format!("dlopen {}", path.display()))?;
        let abi_fn: Symbol<AbiVersionFn> = unsafe { lib.get(b"wiimaker_play_abi_version") }
            .context("missing wiimaker_play_abi_version")?;
        let abi = unsafe { abi_fn() };
        if abi != PLAY_ABI_VERSION {
            bail!(
                "play plugin ABI {abi} != editor {PLAY_ABI_VERSION} ({})",
                path.display()
            );
        }
        let create: Symbol<CreateFn> =
            unsafe { lib.get(b"wiimaker_play_create") }.context("missing wiimaker_play_create")?;
        let destroy: Symbol<DestroyFn> = unsafe { lib.get(b"wiimaker_play_destroy") }
            .context("missing wiimaker_play_destroy")?;
        let update: Symbol<UpdateFn> =
            unsafe { lib.get(b"wiimaker_play_update") }.context("missing wiimaker_play_update")?;
        let render: Symbol<RenderFn> =
            unsafe { lib.get(b"wiimaker_play_render") }.context("missing wiimaker_play_render")?;
        let world: Symbol<WorldFn> =
            unsafe { lib.get(b"wiimaker_play_world") }.context("missing wiimaker_play_world")?;

        let create_fn = *create;
        let destroy_fn = *destroy;
        let update_fn = *update;
        let render_fn = *render;
        let world_fn = *world;

        let dir_c =
            CString::new(game_dir.to_string_lossy().as_bytes()).context("game_dir contains NUL")?;
        let json_c = scene_json
            .filter(|s| !s.is_empty())
            .map(|s| CString::new(s).context("scene JSON contains NUL"))
            .transpose()?;
        let json_ptr = json_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(std::ptr::null());
        let handle = unsafe { create_fn(dir_c.as_ptr(), json_ptr) };
        if handle.is_null() {
            bail!("wiimaker_play_create returned null ({})", path.display());
        }

        Ok(Self {
            handle,
            abi,
            create: create_fn,
            destroy: destroy_fn,
            update: update_fn,
            render: render_fn,
            world: world_fn,
            _lib: lib,
        })
    }

    pub fn update(&mut self, input: &Input, dt: f32, fb_w: u32, fb_h: u32) -> Result<()> {
        let pad = PlayInputC::from_input(input);
        let rc = unsafe { (self.update)(self.handle, &pad, dt, fb_w, fb_h) };
        if rc != 0 {
            bail!("play plugin update failed ({rc})");
        }
        Ok(())
    }

    pub fn render(
        &mut self,
        draw: &mut DrawList,
        input: &Input,
        dt: f32,
        fb_w: u32,
        fb_h: u32,
    ) -> Result<()> {
        let pad = PlayInputC::from_input(input);
        let rc = unsafe { (self.render)(self.handle, draw, &pad, dt, fb_w, fb_h) };
        if rc != 0 {
            bail!("play plugin render failed ({rc})");
        }
        Ok(())
    }

    pub fn world(&self) -> Option<&World> {
        let p = unsafe { (self.world)(self.handle) };
        if p.is_null() {
            None
        } else {
            Some(unsafe { &*p })
        }
    }

    pub fn world_mut(&mut self) -> Option<&mut World> {
        let p = unsafe { (self.world)(self.handle) };
        if p.is_null() {
            None
        } else {
            Some(unsafe { &mut *p })
        }
    }
}

impl std::fmt::Debug for LoadedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedPlugin")
            .field("abi", &self.abi)
            .field("handle", &self.handle)
            .finish()
    }
}
