# Assets

`just env` bakes the studio in `source/` into what the renderer embeds.

| File | Description |
|---|---|
| `source/wooden_studio_16_1k.hdr` | The studio, as a 1k latitude-longitude HDRI |
| `specular.bin` | The studio's specular cube map, baked |

The diffuse light of the studio is baked into `src/renderer/environment.rs`.

## Studio

[Wooden Studio 16](https://polyhaven.com/a/wooden_studio_16) by Alexander Scholten, from Poly
Haven, under CC0, chosen from its studio category. The 1k file is
<https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/1k/wooden_studio_16_1k.hdr>.

- One round softbox with nearly half of its light, which the bake turns to `Light::default().toward`
- A white diffusion ceiling and white bounce flats as the weaker fill
- Black walls and floor: flat faces mirror the ceiling, and the sides of things the floor
