# potpot

panels for audio plugins drawn as a lit 3d scene, live on the gpu. knobs, faders, buttons,
lamps and lettering are geometry under one light, so shadows, shine and the spill of a lamp
come from the scene and nothing is rendered ahead of time. for plugins whose editor is a
[baseview](https://github.com/RustAudio/baseview) window.

three crates:

- `potpot` draws a scene of parts with wgpu, and says what part is under a point
- `potpot-controls` is what a panel's controls do, with no look: dragging, fine and stepped
  turns, resetting, typing a value, the menu. it depends on nothing
- `potpot-window` is a baseview window for a panel of controls, drawn by whatever skin it
  is given, and an editor for [nice-plug](https://crates.io/crates/nice-plug)

`potpot-controls` and `potpot-window` do not need `potpot`: `just flat` opens a panel drawn with
flat shapes alone. `just picture out.png synth` draws one of potpot's example panels to a
file.

vulkan, direct3d 12 and metal only.
