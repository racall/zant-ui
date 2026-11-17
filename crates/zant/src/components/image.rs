//! Image loading, intrinsic size and paint (fit + radius clipping).
//!
//! The retained tree deliberately does not do file I/O
//! (`core/tree/alloc.rs:add_image`), so the runtime calls [`load`] when it
//! creates an image node or when its `path` changes — in the reconcile step,
//! outside the render pass.
//!
//! `zant_image_load_file` (Skia file codec) is not called directly because
//! `unsafe` is not allowed in `components/`; [`load`] reads the bytes with
//! `std::fs` and decodes them through the safe [`Image::from_encoded`] wrapper
//! (same Skia codec, eager rather than deferred decode).
#![forbid(unsafe_code)]

use std::rc::Rc;

use crate::core::node::{FlexStyle, ImageData, NodeData, RetainedNode};
use crate::image::Image;
use crate::renderer::canvas::Canvas;
use crate::theme::tokens::Theme;

/// The fallback width before a source is loaded.
pub const DEFAULT_WIDTH: f32 = 160.0;
/// The fallback height before a source is loaded.
pub const DEFAULT_HEIGHT: f32 = 120.0;

/// Loads (or reloads) [`ImageData::path`] into the node.
///
/// The handle and the intrinsic size are reset even
/// when the file cannot be read or decoded (a placeholder is painted then).
/// Call it from the runtime's reconcile step, never from `render`.
pub fn load(data: &mut ImageData) {
    data.handle = None;
    data.intrinsic_width = 0;
    data.intrinsic_height = 0;
    let Ok(bytes) = std::fs::read(&*data.path) else {
        return;
    };
    let Some(image) = Image::from_encoded(&bytes) else {
        return;
    };
    data.intrinsic_width = image.width as i32;
    data.intrinsic_height = image.height as i32;
    data.handle = Some(Rc::new(image));
}

/// Shows an in-memory image, keeping its own reference to the pixels (the
/// `Rc` clone).
///
/// `source` is `Option<&Rc<Image>>` so the runtime can pass
/// `element.image_handle.as_ref()` without consuming the element; `id` is the
/// `Image::id` of the source (`element.image_id`, 0 = file path).
pub fn set_source(data: &mut ImageData, source: Option<&Rc<Image>>, id: u64) {
    data.handle = None;
    data.intrinsic_width = 0;
    data.intrinsic_height = 0;
    data.source_id = id;
    let Some(source) = source else {
        return;
    };
    data.handle = Some(Rc::clone(source));
    data.intrinsic_width = source.width as i32;
    data.intrinsic_height = source.height as i32;
}

/// Releases the handle; the dropped `Rc` frees the pixels
/// when the last node stops showing them.
pub fn destroy(data: &mut ImageData) {
    data.handle = None;
}

/// The intrinsic width, or 160px before it is known.
pub fn measure_width(style: &FlexStyle, data: &ImageData) -> f32 {
    if let Some(width) = style.width {
        return width;
    }
    if data.intrinsic_width > 0 {
        data.intrinsic_width as f32
    } else {
        DEFAULT_WIDTH
    }
}

/// Keeps the intrinsic aspect ratio at `width`.
pub fn measure_height(style: &FlexStyle, data: &ImageData, width: f32) -> f32 {
    if let Some(height) = style.height {
        return height;
    }
    if data.intrinsic_width > 0 && data.intrinsic_height > 0 {
        width * data.intrinsic_height as f32 / data.intrinsic_width as f32
    } else {
        DEFAULT_HEIGHT
    }
}

/// Paints the pixels clipped to the node radius, or the
/// fill-secondary placeholder while unloaded / undecodable.
pub fn paint(node: &mut RetainedNode, canvas: Canvas, theme: Theme) {
    let rect = node.bounds;
    let visual = node.visual;
    let data = match &node.data {
        NodeData::Image(data) => data,
        _ => return,
    };
    let Some(handle) = data.handle.as_ref() else {
        // placeholder until the image loads (or when it cannot be decoded)
        canvas.fill_round_rect(rect, visual.radius, theme.color_fill_secondary);
        return;
    };
    handle.draw_rect(canvas, rect, visual.radius, data.fit as i32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Str;
    use crate::core::geometry::Rect;
    use crate::core::node::{ImageData, ImageFit};
    use crate::image::{Image, NATIVE_ORDER, PixelOrder};
    use crate::renderer::renderer::Renderer;
    use crate::theme::tokens::ant_light;

    const CLEAR: u32 = 0xff00_ff00;
    const RED: u32 = 0xffff_0000;
    const WIDTH: usize = 100;
    const HEIGHT: usize = 80;

    /// A solid red image (40×20 unless `height` says otherwise).
    fn red_image(width: u32, height: u32) -> Image {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        for pixel in pixels.chunks_exact_mut(4) {
            match NATIVE_ORDER {
                PixelOrder::Rgba => pixel.copy_from_slice(&[255, 0, 0, 255]),
                PixelOrder::Bgra => pixel.copy_from_slice(&[0, 0, 255, 255]),
            }
        }
        Image::from_pixels(&pixels, width, height, width * 4).expect("red image")
    }

    fn image_node(fit: ImageFit, radius: f32) -> RetainedNode {
        let mut node = RetainedNode::new(
            0,
            NodeData::Image(ImageData {
                fit,
                radius,
                ..Default::default()
            }),
        );
        node.visual.radius = radius;
        node.bounds = Rect::init(10.0, 10.0, 40.0, 20.0);
        node
    }

    fn image_data(node: &RetainedNode) -> &ImageData {
        match &node.data {
            NodeData::Image(data) => data,
            _ => panic!("not an image node"),
        }
    }

    fn image_data_mut(node: &mut RetainedNode) -> &mut ImageData {
        match &mut node.data {
            NodeData::Image(data) => data,
            _ => panic!("not an image node"),
        }
    }

    fn paint_into(node: &mut RetainedNode, theme: Theme) -> Renderer {
        let mut renderer =
            Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            paint(node, canvas, theme);
        }
        renderer.end();
        renderer
    }

    #[test]
    fn load_reads_the_encoded_file_and_its_intrinsic_size() {
        let source = red_image(40, 20);
        let png = source.encode_png().expect("png");
        let path =
            std::env::temp_dir().join(format!("zant_wp22_image_load_{}.png", std::process::id()));
        std::fs::write(&path, &png).expect("write png");
        let mut data = ImageData {
            path: Str::copy(path.to_str().expect("utf-8 path")),
            ..Default::default()
        };
        load(&mut data);
        assert!(data.handle.is_some());
        assert_eq!((data.intrinsic_width, data.intrinsic_height), (40, 20));
        // a reload with a missing file clears the state (placeholder is painted)
        std::fs::remove_file(&path).expect("remove png");
        load(&mut data);
        assert!(data.handle.is_none());
        assert_eq!((data.intrinsic_width, data.intrinsic_height), (0, 0));
    }

    #[test]
    fn set_source_keeps_a_reference_to_the_in_memory_image() {
        let image = Rc::new(red_image(40, 20));
        let mut data = ImageData::default();
        set_source(&mut data, Some(&image), 77);
        assert!(data.handle.is_some());
        assert_eq!(data.source_id, 77);
        assert_eq!((data.intrinsic_width, data.intrinsic_height), (40, 20));
        // dropping the caller's handle keeps the node's pixels alive
        drop(image);
        assert_eq!(data.handle.as_ref().map(|image| image.width), Some(40));
        // `None` shows a file image again
        set_source(&mut data, None, 0);
        assert!(data.handle.is_none());
        assert_eq!(data.source_id, 0);
        assert_eq!((data.intrinsic_width, data.intrinsic_height), (0, 0));
    }

    #[test]
    fn destroy_releases_the_handle() {
        let mut data = ImageData::default();
        set_source(&mut data, Some(&Rc::new(red_image(40, 20))), 1);
        destroy(&mut data);
        assert!(data.handle.is_none());
    }

    #[test]
    fn measure_width_and_height_follow_the_intrinsic_size() {
        let style = FlexStyle::default();
        let mut data = ImageData {
            intrinsic_width: 40,
            intrinsic_height: 20,
            ..Default::default()
        };
        assert_eq!(measure_width(&style, &data), 40.0);
        assert_eq!(measure_height(&style, &data, 100.0), 50.0);
        // no source yet: the antd defaults
        data.intrinsic_width = 0;
        data.intrinsic_height = 0;
        assert_eq!(measure_width(&style, &data), DEFAULT_WIDTH);
        assert_eq!(measure_height(&style, &data, 100.0), DEFAULT_HEIGHT);
        // explicit style sizes win
        let sized = FlexStyle {
            width: Some(300.0),
            height: Some(120.0),
            ..Default::default()
        };
        assert_eq!(measure_width(&sized, &data), 300.0);
        assert_eq!(measure_height(&sized, &data, 10.0), 120.0);
    }

    #[test]
    fn paint_draws_the_image_and_falls_back_to_the_placeholder() {
        let theme = ant_light();
        let mut node = image_node(ImageFit::Fill, 0.0);
        let image = Rc::new(red_image(40, 20));
        let data = image_data_mut(&mut node);
        data.handle = Some(Rc::clone(&image));
        data.intrinsic_width = 40;
        data.intrinsic_height = 20;
        let painted = paint_into(&mut node, theme);
        assert_eq!(painted.pixel_at(30, 20), Some(RED));

        // no handle: the fill-secondary placeholder, not the image
        let mut placeholder = image_node(ImageFit::Fill, 0.0);
        let blank = paint_into(&mut placeholder, theme);
        let pixel = blank.pixel_at(30, 20).expect("pixel");
        assert_ne!(pixel, CLEAR);
        assert_ne!(pixel, RED);
    }

    #[test]
    fn paint_clips_the_image_to_the_radius() {
        let theme = ant_light();
        let mut node = image_node(ImageFit::Fill, 8.0);
        node.bounds = Rect::init(10.0, 10.0, 40.0, 40.0);
        let data = image_data_mut(&mut node);
        data.handle = Some(Rc::new(red_image(40, 40)));
        data.intrinsic_width = 40;
        data.intrinsic_height = 40;
        let painted = paint_into(&mut node, theme);
        // the rounded corner leaves the clear colour visible
        assert_eq!(painted.pixel_at(10, 10), Some(CLEAR));
        assert_eq!(painted.pixel_at(30, 30), Some(RED));
    }

    #[test]
    fn paint_honours_fill_cover_and_contain() {
        let theme = ant_light();
        let render = |fit: ImageFit| {
            let mut node = image_node(fit, 0.0);
            node.bounds = Rect::init(10.0, 10.0, 40.0, 40.0);
            let data = image_data_mut(&mut node);
            data.handle = Some(Rc::new(red_image(40, 20)));
            data.intrinsic_width = 40;
            data.intrinsic_height = 20;
            paint_into(&mut node, theme)
        };
        // fill stretches 40×20 over the 40×40 box
        assert_eq!(render(ImageFit::Fill).pixel_at(30, 15), Some(RED));
        // cover crops: the whole box is covered
        assert_eq!(render(ImageFit::Cover).pixel_at(30, 15), Some(RED));
        // contain letterboxes: the top / bottom of the box stay clear
        let contained = render(ImageFit::Contain);
        assert_eq!(contained.pixel_at(30, 12), Some(CLEAR));
        assert_eq!(contained.pixel_at(30, 30), Some(RED));
    }

    #[test]
    fn tree_paint_dispatches_to_the_image_module() {
        use crate::core::geometry::Size;
        use crate::core::tree::Tree;
        let theme = ant_light();
        let mut tree = Tree::init(theme);
        let root = tree.root;
        tree.get_mut(root).style.direction = crate::flex::Direction::Row;
        let id = tree.add_image(Some(root), "missing.png", ImageFit::Fill, 0.0);
        let data = image_data_mut(tree.get_mut(id));
        data.handle = Some(Rc::new(red_image(40, 20)));
        data.intrinsic_width = 40;
        data.intrinsic_height = 20;
        tree.layout(Size {
            width: WIDTH as f32,
            height: HEIGHT as f32,
        });
        let mut renderer =
            Renderer::create_raster(WIDTH as u32, HEIGHT as u32).expect("raster renderer");
        {
            let canvas = renderer.begin(1.0).expect("frame canvas");
            canvas.clear(CLEAR);
            tree.paint(canvas);
        }
        renderer.end();
        // the image is laid out at (0, 0, 40, 20) from its intrinsic size
        assert_eq!(renderer.pixel_at(20, 10), Some(RED));
    }

    #[test]
    fn paint_of_a_non_image_node_is_a_no_op() {
        let theme = ant_light();
        let mut node = RetainedNode::new(0, NodeData::Root);
        node.bounds = Rect::init(0.0, 0.0, 20.0, 20.0);
        let painted = paint_into(&mut node, theme);
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                assert_eq!(painted.pixel_at(x, y), Some(CLEAR));
            }
        }
    }

    #[test]
    fn fit_codes_match_the_native_image_bridge() {
        // skia_bridge.cpp: cover = 0, contain = 1, fill = anything else
        assert_eq!(ImageFit::Cover as i32, 0);
        assert_eq!(ImageFit::Contain as i32, 1);
        assert_eq!(ImageFit::Fill as i32, 2);
        assert_eq!(
            image_data(&image_node(ImageFit::Cover, 0.0)).fit,
            ImageFit::Cover
        );
    }
}
