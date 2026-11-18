# Rupert's icon

The chosen concept is `rupert-owl-wireframe-glasses.png` (see `CLAUDE-HANDOFF.md`). It is a preview: the tile sits on a light presentation background.

The production masters are cut from it:

- `rupert-icon-desktop-1024.png`: transparent, the tile at 824 of 1024 px with a soft shadow (Apple's macOS grid). Source of `src-tauri/icons/` and, at 128 px with less padding, `static/favicon.png`.
- `rupert-icon-ios-1024.png`: full bleed and opaque, zoomed 4.5 % so iOS's own mask never shows the concept's rounder corners. Source of `mobile/icons/` and the Xcode icon set.

To rebuild them:

```sh
swift scripts/icons/make-icons.swift docs/icon-concepts/rupert-owl-wireframe-glasses.png <out-dir>
npx tauri icon <out-dir>/rupert-icon-desktop-1024.png -o <tmp>        # copy the files src-tauri/icons already has
npx tauri icon <out-dir>/rupert-icon-ios-1024.png -o <tmp2>           # mobile/icons, and <tmp2>/ios → the AppIcon set
swift scripts/icons/flatten-alpha.swift mobile/gen/apple/Assets.xcassets/AppIcon.appiconset/*.png
sips -z 128 128 <out-dir>/rupert-favicon-512.png --out static/favicon.png
```

The script measures where the presentation background ends on every row and column and cuts 5 px inside that edge, past the concept's light bevel.
