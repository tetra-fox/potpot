// the light of `Light::default`, which `scene.rs` includes and the xtask's studio bake turns
// the studio's softbox onto. it is a file of its own so that the bake builds without potpot

// the direction from the panel toward the light
const DEFAULT_TOWARD: [f32; 3] = [-0.45, -0.6, 1.0];
// the light's color and brightness, as linear light
const DEFAULT_COLOR: [f32; 3] = [1.0, 1.0, 1.0];
