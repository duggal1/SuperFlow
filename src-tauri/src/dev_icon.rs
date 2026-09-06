//! Dock identity painting (macOS).
//!
//! Dev runs (`tauri dev`) execute an unbundled binary — no `.app`, so macOS
//! would show a generic executable icon and process name. Packaged builds
//! already carry the product `icon.icns`, which is also what the Dock shows
//! while the app is pinned but not running — so they must keep it while
//! running too. Painting a second tile at runtime with different padding or
//! scale would make the running Dock icon look larger/fatter than the pinned
//! one, therefore the paint below only applies to unbundled (dev) runs.
//!
//! The SVG is compiled into the binary, so nothing depends on the source
//! checkout existing at runtime.

#![cfg(target_os = "macos")]

use objc2::AnyThread;
use objc2_app_kit::{NSApplication, NSBezierPath, NSColor, NSCompositingOperation, NSImage};
use objc2_foundation::{ns_string, NSBundle, NSData, NSRect, NSSize, NSString};

/// Compiled in from the repo's public/logo.svg — single source of truth for
/// the mark across webview and native chrome.
const LOGO_SVG: &str = include_str!("../../public/logo.svg");

const TILE: f64 = 1024.0;
/// Apple's squircle corner ratio for app tiles.
const CORNER_RADIUS_RATIO: f64 = 0.2237;
/// Dev-tile background (stone-950 #0c0a09) for unbundled runs.
const STONE_950: (f64, f64, f64) = (
    0x0c as f64 / 255.0,
    0x0a as f64 / 255.0,
    0x09 as f64 / 255.0,
);
/// Logo occupies ~54% of the tile, optically centered.
const LOGO_SCALE: f64 = 0.54;

/// Best-effort display-name override so menus say "SuperFlow" even when the
/// binary runs unbundled. Silently does nothing when the runtime info
/// dictionary refuses mutation (packaged builds already carry real values).
fn apply_display_name() {
    unsafe {
        let Some(info) = NSBundle::mainBundle().infoDictionary() else {
            return;
        };
        let name = NSString::from_str("SuperFlow");
        let _: () = objc2::msg_send![
            &info,
            setObject: &*name,
            forKey: ns_string!("CFBundleName")
        ];
        let _: () = objc2::msg_send![
            &info,
            setObject: &*name,
            forKey: ns_string!("CFBundleDisplayName")
        ];
    }
}

/// True when the current executable runs from inside a packaged `.app`
/// bundle (`…/SuperFlow.app/Contents/MacOS/superflow`). Unbundled dev runs
/// (`target/debug/superflow`) have no `.app` ancestor.
fn is_bundled_app() -> bool {
    std::env::current_exe().is_ok_and(|exe| {
        exe.components()
            .any(|component| component.as_os_str().to_string_lossy().ends_with(".app"))
    })
}

pub fn apply() {
    apply_display_name();

    // A packaged run lives inside `SuperFlow.app/Contents/MacOS/…` and already
    // shows `icon.icns` in the Dock while pinned. Overriding it here with a
    // runtime-painted tile (full-bleed 1024 canvas, stone-950, 54% mark) uses
    // different padding/scale than the bundled set, so the icon visibly grows
    // fatter the moment the app starts and shrinks back on quit. Skip the
    // paint when bundled so running and pinned share the exact same asset;
    // unbundled dev runs still get the painted identity below.
    if is_bundled_app() {
        return;
    }

    unsafe {
        let data = NSData::with_bytes(LOGO_SVG.as_bytes());
        let Some(logo) = NSImage::initWithData(NSImage::alloc(), &data) else {
            return;
        };

        #[allow(deprecated)] // lockFocus is soft-deprecated but ideal here
        {
            let container = NSImage::initWithSize(NSImage::alloc(), NSSize::new(TILE, TILE));

            container.lockFocus();

            let bg = NSColor::colorWithSRGBRed_green_blue_alpha(
                STONE_950.0,
                STONE_950.1,
                STONE_950.2,
                1.0,
            );
            bg.setFill();
            let radius = TILE * CORNER_RADIUS_RATIO;
            let tile_path = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                NSRect::new(
                    objc2_foundation::NSPoint::new(0.0, 0.0),
                    NSSize::new(TILE, TILE),
                ),
                radius,
                radius,
            );
            tile_path.fill();

            let logo_size = TILE * LOGO_SCALE;
            let origin = (TILE - logo_size) / 2.0;
            logo.drawInRect_fromRect_operation_fraction(
                NSRect::new(
                    objc2_foundation::NSPoint::new(origin, origin),
                    NSSize::new(logo_size, logo_size),
                ),
                NSRect::ZERO,
                NSCompositingOperation::SourceOver,
                1.0,
            );

            container.unlockFocus();

            NSApplication::sharedApplication(objc2::MainThreadMarker::new_unchecked())
                .setApplicationIconImage(Some(&container));
        }
    }
}
