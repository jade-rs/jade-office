//! SmartArt as PowerPoint for Mac's SmartArt のデザイン and 書式 tabs and
//! its テキスト ウィンドウ edit it: insert a diagram from the gallery, type
//! its bullets, add, delete, promote, demote and move shapes, add an
//! assistant, change its layout, colours and style, format one of its
//! shapes, reset it, and convert between bulleted text, diagrams and plain
//! shapes. Both shells call these; each step is one Undo.

use jade_pptx_reader::Transform;
pub use jade_pptx_writer::{SmartArtEdit, SmartArtInfo, SmartArtShape};
pub use jade_smartart::catalog::{
    Category as SmartArtCategory, ColorScheme as SmartArtColors, LayoutInfo as SmartArtLayout,
    Style as SmartArtStyle, COLOR_SCHEMES as SMARTART_COLORS, LAYOUTS as SMARTART_LAYOUTS,
    STYLES as SMARTART_STYLES,
};
pub use jade_smartart::tree::Place as SmartArtPlace;
pub use jade_smartart::{
    Node as SmartArtNode, NodeKind as SmartArtNodeKind, ShapeFormat as SmartArtFormat,
};

use super::{Deck, ShapeRect, EMU_PER_PX};
use crate::Backend;

/// Bullets a new diagram of `layout` starts with, as PowerPoint's: empty
/// ones, the text pane prompting `[テキスト]` in each.
fn starter(layout: &str) -> Vec<SmartArtNode> {
    let short = layout.rsplit('/').next().unwrap_or(layout);
    match short {
        "default" | "pList1" | "TitledPictureBlocks" | "CaptionedPictures" => {
            (0..5).map(|_| SmartArtNode::new("")).collect()
        }
        "orgChart1" | "NameandTitleOrganizationalChart" | "hierarchy1" | "hierarchy2" => {
            vec![SmartArtNode {
                children: vec![
                    SmartArtNode::assistant(""),
                    SmartArtNode::new(""),
                    SmartArtNode::new(""),
                    SmartArtNode::new(""),
                ]
                .into_iter()
                .filter(|node| {
                    node.kind != SmartArtNodeKind::Assistant
                        || short.contains("org")
                        || short.contains("Organizational")
                })
                .collect(),
                ..SmartArtNode::new("")
            }]
        }
        "radial1" | "matrix1" => vec![SmartArtNode::with(
            "",
            (0..4).map(|_| SmartArtNode::new("")).collect(),
        )],
        "matrix2" | "matrix3" | "cycle3" | "venn2" => {
            (0..4).map(|_| SmartArtNode::new("")).collect()
        }
        "cycle1" | "cycle2" | "chart3" => (0..5).map(|_| SmartArtNode::new("")).collect(),
        _ => (0..3).map(|_| SmartArtNode::new("")).collect(),
    }
}

/// The text pane's lines: each bullet's level and text.
pub type TextPane = Vec<(usize, String)>;

impl<B: Backend> Deck<B> {
    fn smartart_edited(&mut self, result: jade_pptx_writer::Result<bool>) -> Result<bool, String> {
        self.after(result)
    }

    fn smartart_made(&mut self, result: jade_pptx_writer::Result<u32>) -> Result<u32, String> {
        let id = result.map_err(|error| error.to_string())?;
        self.reload()?;
        Ok(id)
    }

    /// The diagram in frame `id`, or `None` when it is no diagram.
    pub fn smartart(&self, slide: usize, id: u32) -> Option<SmartArtInfo> {
        jade_pptx_writer::smartart(self.session.bytes(), slide, id)
            .ok()
            .flatten()
    }

    /// 挿入 › SmartArt: a diagram of `layout` (an id or a short name) in the
    /// middle of the slide, two thirds of its width and four fifths of its
    /// height as PowerPoint places one. Its frame id.
    pub fn insert_smartart(&mut self, slide: usize, layout: &str) -> Result<u32, String> {
        let (width, height) = self.slide_size();
        let rect = ShapeRect {
            x: width / 6.0,
            y: height * 0.105,
            width: width * 2.0 / 3.0,
            height: height * 0.79,
        };
        let layout = jade_smartart::catalog::layout_id(layout);
        let nodes = starter(&layout);
        let result = self
            .session
            .insert_smartart(slide, frame(rect), &layout, &nodes);
        self.smartart_made(result)
    }

    fn smartart_nodes(&self, slide: usize, id: u32) -> Result<Vec<SmartArtNode>, String> {
        self.smartart(slide, id)
            .map(|info| info.nodes)
            .ok_or_else(|| "not a SmartArt graphic".to_string())
    }

    fn set_smartart_nodes(
        &mut self,
        slide: usize,
        id: u32,
        nodes: Vec<SmartArtNode>,
    ) -> Result<bool, String> {
        let result = self
            .session
            .edit_smartart(slide, id, &SmartArtEdit::Nodes(nodes));
        self.smartart_edited(result)
    }

    /// The テキスト ウィンドウ's lines.
    pub fn smartart_text_pane(&self, slide: usize, id: u32) -> Option<TextPane> {
        let info = self.smartart(slide, id)?;
        Some(
            jade_smartart::data::outline(&info.nodes)
                .into_iter()
                .map(|(level, node)| (level, node.text.clone()))
                .collect(),
        )
    }

    /// The テキスト ウィンドウ edited: its lines as they now stand.
    pub fn set_smartart_text_pane(
        &mut self,
        slide: usize,
        id: u32,
        lines: &[(usize, String)],
    ) -> Result<bool, String> {
        let nodes = self.smartart_nodes(slide, id)?;
        self.set_smartart_nodes(
            slide,
            id,
            jade_smartart::tree::from_text_pane(&nodes, lines),
        )
    }

    /// 図形の追加 at `place` relative to bullet `node` (or after the last
    /// when none is selected): the new bullet's id.
    pub fn smartart_add_shape(
        &mut self,
        slide: usize,
        id: u32,
        node: Option<&str>,
        place: SmartArtPlace,
    ) -> Result<Option<String>, String> {
        let nodes = self.smartart_nodes(slide, id)?;
        let before: Vec<String> = ids(&nodes);
        let changed = jade_smartart::tree::add(&nodes, node, place, SmartArtNode::new(""));
        self.set_smartart_nodes(slide, id, changed)?;
        let after = ids(&self.smartart_nodes(slide, id)?);
        Ok(after.into_iter().find(|id| !before.contains(id)))
    }

    /// Deletes bullet `node`'s shape; the bullets under it move up.
    pub fn smartart_delete_shape(
        &mut self,
        slide: usize,
        id: u32,
        node: &str,
    ) -> Result<bool, String> {
        let nodes = self.smartart_nodes(slide, id)?;
        self.set_smartart_nodes(slide, id, jade_smartart::tree::remove(&nodes, node))
    }

    /// レベル上げ.
    pub fn smartart_promote(&mut self, slide: usize, id: u32, node: &str) -> Result<bool, String> {
        let nodes = self.smartart_nodes(slide, id)?;
        self.set_smartart_nodes(slide, id, jade_smartart::tree::promote(&nodes, node))
    }

    /// レベル下げ.
    pub fn smartart_demote(&mut self, slide: usize, id: u32, node: &str) -> Result<bool, String> {
        let nodes = self.smartart_nodes(slide, id)?;
        self.set_smartart_nodes(slide, id, jade_smartart::tree::demote(&nodes, node))
    }

    /// 上へ移動 / 下へ移動.
    pub fn smartart_move(
        &mut self,
        slide: usize,
        id: u32,
        node: &str,
        up: bool,
    ) -> Result<bool, String> {
        let nodes = self.smartart_nodes(slide, id)?;
        self.set_smartart_nodes(
            slide,
            id,
            jade_smartart::tree::move_sibling(&nodes, node, up),
        )
    }

    /// レイアウト, 色の変更, SmartArt のスタイル, one shape's format or
    /// グラフィックのリセット.
    pub fn edit_smartart(
        &mut self,
        slide: usize,
        id: u32,
        edit: &SmartArtEdit,
    ) -> Result<bool, String> {
        let result = self.session.edit_smartart(slide, id, edit);
        self.smartart_edited(result)
    }

    /// 図の挿入 into bullet `node`'s picture shape.
    pub fn set_smartart_picture(
        &mut self,
        slide: usize,
        id: u32,
        node: &str,
        image: &[u8],
    ) -> Result<bool, String> {
        let result = self.session.set_smartart_picture(slide, id, node, image);
        self.smartart_edited(result)
    }

    /// 図形に変換: the group the diagram becomes.
    pub fn convert_smartart_to_shapes(&mut self, slide: usize, id: u32) -> Result<u32, String> {
        let result = self.session.convert_smartart_to_shapes(slide, id);
        self.smartart_made(result)
    }

    /// SmartArt グラフィックに変換 on a text shape: the new frame.
    pub fn convert_to_smartart(
        &mut self,
        slide: usize,
        id: u32,
        layout: &str,
    ) -> Result<u32, String> {
        let layout = jade_smartart::catalog::layout_id(layout);
        let result = self.session.convert_text_to_smartart(slide, id, &layout);
        self.smartart_made(result)
    }

    /// The bullet whose shape is at slide point (`x`, `y`) inside diagram
    /// `id` — what a second click in a diagram selects.
    pub fn smartart_node_at(&mut self, slide: usize, id: u32, x: f64, y: f64) -> Option<String> {
        let frame = self.shape(slide, id)?.transform?;
        let info = self.smartart(slide, id)?;
        let fx = x * EMU_PER_PX - frame.offset_emu.0 as f64;
        let fy = y * EMU_PER_PX - frame.offset_emu.1 as f64;
        info.shapes
            .iter()
            .rev()
            .filter(|shape| !shape.nodes.is_empty())
            .find(|shape| {
                let (sx, sy, sw, sh) = shape.rect;
                fx >= sx as f64
                    && fx <= (sx + sw) as f64
                    && fy >= sy as f64
                    && fy <= (sy + sh) as f64
            })
            .map(|shape| shape.nodes[0].clone())
    }

    /// The box (slide px) of the shape that shows bullet `node`: where the
    /// selection of one shape of a diagram is drawn.
    pub fn smartart_node_rect(&mut self, slide: usize, id: u32, node: &str) -> Option<ShapeRect> {
        let frame = self.shape(slide, id)?.transform?;
        let info = self.smartart(slide, id)?;
        let shape = info
            .shapes
            .iter()
            .find(|shape| shape.nodes.first().map(String::as_str) == Some(node))?;
        let (sx, sy, sw, sh) = shape.rect;
        Some(ShapeRect {
            x: (frame.offset_emu.0 + sx) as f64 / EMU_PER_PX,
            y: (frame.offset_emu.1 + sy) as f64 / EMU_PER_PX,
            width: sw as f64 / EMU_PER_PX,
            height: sh as f64 / EMU_PER_PX,
        })
    }
}

fn ids(nodes: &[SmartArtNode]) -> Vec<String> {
    jade_smartart::data::outline(nodes)
        .into_iter()
        .map(|(_, node)| node.id.clone())
        .collect()
}

fn frame(rect: ShapeRect) -> Transform {
    rect.apply_to(Transform {
        offset_emu: (0, 0),
        extent_emu: (0, 0),
        rotation_60000ths: 0,
        flip_h: false,
        flip_v: false,
    })
}
