| Hammertone | Made by the bake |# Assets
| Hammertone | Made by the bake |
| Hammertone | Made by the bake |`just assets` bakes what is in `source/` into what the renderer embeds, and fails if that
| Hammertone | Made by the bake |comes to more than 8 MB. `just env` bakes the studio alone.
| Hammertone | Made by the bake |
| Hammertone | Made by the bake || File | Description |
| Hammertone | Made by the bake ||---|---|
| Hammertone | Made by the bake || `source/wooden_studio_16_1k.hdr` | The studio, as a 1k latitude-longitude HDRI |
| Hammertone | Made by the bake || `studio.bin` | The specular cube map of the studio without its softbox, baked |
| Hammertone | Made by the bake || `softbox.bin` | The specular cube map of the softbox alone, baked |
| Hammertone | Made by the bake || `source/<ID>_1K-JPG_*.jpg` | The color, normal and roughness of the photographed sets |
| Hammertone | Made by the bake || `albedo.bin` | The sets' color maps as BC7, baked at 512 by 512 with every mip |
| Hammertone | Made by the bake || `normal.bin` | The sets' normal maps as BC5, laid out as `albedo.bin` is |
| Hammertone | Made by the bake || `roughness.bin` | The sets' roughness maps as BC4, laid out as `albedo.bin` is |
| Hammertone | Made by the bake |
| Hammertone | Made by the bake |The diffuse light of the studio is baked into `src/renderer/environment.rs`, and the sets'
| Hammertone | Made by the bake |means into `src/renderer/textures.rs`.
| Hammertone | Made by the bake |
| Hammertone | Made by the bake |## Studio
| Hammertone | Made by the bake |
| Hammertone | Made by the bake |[Wooden Studio 16](https://polyhaven.com/a/wooden_studio_16) by Alexander Scholten, from Poly
| Hammertone | Made by the bake |Haven, under CC0, chosen from its studio category. The 1k file is
| Hammertone | Made by the bake |<https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/1k/wooden_studio_16_1k.hdr>.
| Hammertone | Made by the bake |
| Hammertone | Made by the bake |- One round softbox with nearly half of its light, which the bake turns to `Light::default().toward`
| Hammertone | Made by the bake |- A white diffusion ceiling and white bounce flats as the weaker fill
| Hammertone | Made by the bake |- Black walls and floor: flat faces mirror the ceiling, and the sides of things the floor
| Hammertone | Made by the bake |
| Hammertone | Made by the bake |## Texture sets
| Hammertone | Made by the bake |
| Hammertone | Made by the bake || Set | Source |
| Hammertone | Made by the bake ||---|---|
| Hammertone | Made by the bake || Walnut | [Wood027](https://ambientcg.com/view?id=Wood027) |
| Hammertone | Made by the bake || Hammertone | Made by the bake |
| Hammertone | Made by the bake || Brushed aluminium | [Metal010](https://ambientcg.com/view?id=Metal010), evened |
| Hammertone | Made by the bake || Powder coat | [Metal027](https://ambientcg.com/view?id=Metal027) |
| Hammertone | Made by the bake |
| Hammertone | Made by the bake |- The ambientCG sets are under CC0, from their 1K JPG downloads
| Hammertone | Made by the bake |- ambientCG has no hammertone, so the bake makes one: broad shallow pits of many sizes,
| Hammertone | Made by the bake |  clustered, between narrow rounded ridges that rise to small peaks where several meet. The
| Hammertone | Made by the bake |  ridge tops are a touch lighter, and `Material::hammertone` draws it as metal, for the
| Hammertone | Made by the bake |  aluminium flake
| Hammertone | Made by the bake |- Metal010's color and roughness are divided by their mean over 24 texels around, which
| Hammertone | Made by the bake |  takes out its stains and keeps the brushing
