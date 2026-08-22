//! Development-mode Dock identity (`tauri dev` runs of the unbundled binary).
//!
//! `tauri dev` executes `target/debug/superflow` directly — no `.app` bundle,
//! so macOS shows a generic executable icon and process name. On debug builds
//! this module paints the Dock tile at startup: an Apple-style rounded square
//! in stone-900 with `public/logo.svg` centered on it, plus a best-effort
//! bundle-name override so menus read "SuperFlow".
//!
//! Release/packaged builds ship real `icon.icns` metadata and skip this.

#![cfg(all(target_os = "macos", debug_assertions))]

use std::path::PathBuf;

use objc2_app_kit::{NSApplication, NSBezierPath, NSColor, NSCompositingOperation, NSImage};
use objc2_foundation::{ns_string, NSBundle, CGPoint, CGRect, CGSize, NSString};

const TILE: f64 = 1024.0;
/// Apple's squircle corner ratio for app tiles.
const CORNER_RADIUS_RATIO: f64 = 0.2237;
/// stone-900 (#1c1917) — matches the app background token.
const STONE_900: (f64, f64, f64) = (0x1c as f64 / 255.0, 0x19 as f64 / 255.0, 0x17 as f64 / 255.0);
/// Logo occupies ~54% of the tile, optically centered.
const LOGO_SCALE: f64 = 0.54;

fn locate_logo() -> Option<PathBuf> {
    // Dev runs from inside `src-tauri`; walk up to the repo's public/logo.svg.
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        candidates.extend(exe.ancestors().map(|a| a.join("public/logo.svg")));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.extend(cwd.ancestors().map(|a| a.join("public/logo.svg")));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Best-effort display-name override so menus say "SuperFlow" instead of the
/// bare process name while unbundled. Silently does nothing when the runtime
/// info dictionary refuses mutation.
fn apply_display_name() {
    unsafe {
        let info = NSBundle::mainBundle().infoDictionary();
        let Some(info) = info else { return };
        let name = NSString::from_str("SuperFlow");
        let _: Option<()> = objc2::msg_send![
            &info,
            setObject: &*name,
            forKey: ns_string!("CFBundleName")
        ];
        let _: Option<()> = objc2::msg_send![
            &info,
            setObject: &*name,
            forKey: ns_string!("CFBundleDisplayName")
        ];
    }
}

pub fn apply() {
    apply_display_name();

    let Some(logo_path) = locate_logo() else {
        return;
    };
    unsafe {
        let path = NSString::from_str(&logo_path.to_string_lossy());
        let Some(logo) = NSImage::initWithContentsOfFile(NSImage::alloc(), &path) else {
            eprintln!("dev_icon: failed to decode {}", logo_path.display());
            return;
        };

        #[allow(deprecated)] // lockFocus is soft-deprecated but ideal here
        {
            let container =
                NSImage::initWithSize(NSImage::alloc(), CGSize::new(TILE, TILE))
                    .expect("NSImage initWithSize");

            container.lockFocus();

            let bg = NSColor::colorWithSRGBRed(STONE_900.0, STONE_900.1, STONE_900.2, 1.0);
            bg.setFill();
            let radius = TILE * CORNER_RADIUS_RATIO;
            let tile_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                CGRect::new(CGPoint::ZERO, CGSize::new(TILE, TILE)),
                radius,
                radius,
            );
            tile_path.fill();

            let logo_size = TILE * LOGO_SCALE;
            let origin = (TILE - logo_size) / 2.0;
            logo.drawAtPoint_fromRect_operation_fraction(
                CGPoint::new(origin, origin),
                CGRect::new(CGPoint::ZERO, CGSize::ZERO),
                NSCompositingOperation::SourceOver,
                1.0,
            );

            container.unlockFocus();

            NSApplication::sharedApplication()
                .setApplicationIconImage(Some(&container));
        }
    }
}
