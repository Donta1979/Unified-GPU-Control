# MangoHud positioning source findings (v0.9)

From user-supplied MangoHud-master.zip:

- `src/overlay_params.h`: offsets are stored as signed `int`.
- `src/overlay_params.cpp`: `parse_offset_x` and `parse_offset_y` call `parse_unsigned`, rejecting negative configuration values. `parse_signed` already exists.
- `src/overlay.cpp`, `position_layer`: adds offsets to several corner and middle-edge anchor positions; there is no true center-center anchor. Top-center and bottom-center branches do not use `offset_x`.
- The provided **experimental source patch** changes the offset parsers to signed. It does **not** implement center-center anchoring, support all placements, or guarantee behavior across Vulkan/OpenGL. It has not been compiled or tested. **Do not apply to installed MangoHud.**
- For a future center-origin feature, a distinct center-center position in the MangoHud renderer and parser may be necessary. Check all supported rendering paths before exposing Apply in the UI.
