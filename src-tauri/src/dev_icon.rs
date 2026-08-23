//! Dock identity painting (macOS).
//!
//! Dev runs (`tauri dev`) execute an unbundled binary — no `.app`, so macOS
//! would show a generic executable icon and process name. Packaged builds
//! carry `icon.icns`, but the product look is the painted tile below, so both
//! paths render the same identity: an Apple-style rounded square in
//! stone-900 with the embedded `logo.svg` centered on it, plus a best-effort
//! bundle-name override while unbundled.
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
/// stone-900 (#1c1917) — matches the app background token.
const STONE_900: (f64, f64, f64) = (
    0x1c as f64 / 255.0,
    0x19 as f64 / 255.0,
    0x17 as f64 / 255.0,
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

pub fn apply() {
    apply_display_name();

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
                STONE_900.0,
                STONE_900.1,
                STONE_900.2,
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
