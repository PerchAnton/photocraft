# PhotoCraft #8762 UI verification

Native offscreen captures of Edit > Keyboard Shortcuts filtered to `Color Picker`.

- Before: upstream main `ff4738810134d7f7e68b49f2e4f231b686282ff3`, with the regression harness added. No picker rows are listed.
- After: `6177dc61e42db0252a6a2ab917129eeb38ac32c9`. Both picker rows appear under Tools, with empty default bindings.

The captures were rendered with egui-kittest's native wgpu renderer (the renderer used by the snapshot example), at 1100 x 760, without opening a desktop window or loading a user document. The before capture is saved before the expected missing-row assertion fails.

To capture the current UI, set `PHOTOCRAFT_PICKER_SHORTCUT_SNAPSHOTS` to an output directory, then run:

```text
cargo test -p photocraft-ui-egui --lib color_picker_ui::shortcut_tests::unbound_picker_rows_are_visible_in_keyboard_shortcuts -- --exact
```

Original capture files: @PerchAnton. MIT OR Apache-2.0, as the repository's original assets; existing UI assets retain their notices in ATTRIBUTION.md.
