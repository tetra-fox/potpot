# Assets

`just assets` bakes what is in `source/` into what the renderer embeds, and fails if that
comes to more than 8 MB. `just env` bakes the studio alone.

| File | Description |
|---|---|
| `source/wooden_studio_16_1k.hdr` | The studio, as a 1k latitude-longitude HDRI |
| `studio.bin` | The specular cube map of the studio without its softbox, baked |
| `softbox.bin` | The specular cube map of the softbox alone, baked |
| `source/<ID>_1K-JPG_*.jpg` | The color, normal and roughness of each texture set |
| `albedo.bin` | The sets' color maps as BC7, baked at 512 by 512 with every mip |
| `normal.bin` | The sets' normal maps as BC5, laid out as `albedo.bin` is |
| `roughness.bin` | The sets' roughness maps as BC4, laid out as `albedo.bin` is |

The diffuse light of the studio is baked into `src/renderer/environment.rs`, and the sets'
means into `src/renderer/textures.rs`.

## Studio

[Wooden Studio 16](https://polyhaven.com/a/wooden_studio_16) by Alexander Scholten, from Poly
Haven, under CC0, chosen from its studio category. The 1k file is
<https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/1k/wooden_studio_16_1k.hdr>.

- One round softbox with nearly half of its light, which the bake turns to `Light::default().toward`
- A white diffusion ceiling and white bounce flats as the weaker fill
- Black walls and floor: flat faces mirror the ceiling, and the sides of things the floor

## Texture sets

All four are from [ambientCG](https://ambientcg.com) under CC0, as their 1K JPG downloads.

| Set | ID | Source |
|---|---|---|
| Walnut | Wood027 | <https://ambientcg.com/view?id=Wood027> |
| Hammertone | Metal044B | <https://ambientcg.com/view?id=Metal044B> |
| Brushed aluminium | Metal010 | <https://ambientcg.com/view?id=Metal010> |
| Powder coat | Metal027 | <https://ambientcg.com/view?id=Metal027> |

ambientCG has no hammertone. Metal044B, a rough metal with a mottled relief of rounded cells,
is the nearest.
